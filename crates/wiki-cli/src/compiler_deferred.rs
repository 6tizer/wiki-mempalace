use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use wiki_core::{Confidence, DomainSchema, EntryType, PageContract, PageId, Scope, WikiPage};
use wiki_kernel::{initial_status_for, LlmWikiEngine, NoopWikiHook};
use wiki_storage::{CanonicalAliasMapping, SqliteRepository};

pub(crate) struct CompilerDeferredResolutionOptions<'a> {
    pub(crate) report_path: &'a Path,
    pub(crate) report_dir: &'a Path,
    pub(crate) apply: bool,
    pub(crate) allow_create: bool,
    pub(crate) scope: &'a Scope,
    pub(crate) schema: &'a DomainSchema,
}

#[derive(Debug)]
pub(crate) struct CompilerDeferredResolutionRun {
    pub(crate) db_changed: bool,
    pub(crate) aliases_applied: usize,
    pub(crate) pages_created: usize,
    pub(crate) changed_page_ids: Vec<PageId>,
    pub(crate) json_report_path: PathBuf,
    pub(crate) markdown_report_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CompilerRunReportInput {
    #[serde(default)]
    deferred_resolutions: Vec<DeferredResolutionInput>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeferredResolutionInput {
    #[serde(default)]
    source_title: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
    draft_title: String,
    draft_entry_type: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    candidates: Vec<DeferredResolutionCandidateInput>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeferredResolutionCandidateInput {
    #[serde(default)]
    page_id: Option<PageId>,
    title: String,
    entry_type: String,
    #[serde(default)]
    score: i32,
    #[serde(default)]
    match_reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct ResolvedDeferredCandidate {
    page_id: PageId,
    title: String,
    score: i32,
    match_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum DeferredDecisionKind {
    AliasExisting,
    CreateCanonical,
    IgnoreNoise,
    KeepDeferred,
}

#[derive(Debug, Clone, Serialize)]
struct DeferredDecisionReport {
    draft_title: String,
    draft_entry_type: String,
    decision: DeferredDecisionKind,
    reason: String,
    confidence: String,
    source_title: Option<String>,
    source_url: Option<String>,
    canonical_page_id: Option<PageId>,
    canonical_title: Option<String>,
    candidates_seen: usize,
    applied: bool,
}

#[derive(Debug, Serialize)]
struct DeferredResolutionAuditReport {
    version: u32,
    kind: &'static str,
    at: String,
    mode: &'static str,
    input_report: String,
    allow_create: bool,
    counts: BTreeMap<String, usize>,
    aliases_applied: usize,
    pages_created: usize,
    decisions: Vec<DeferredDecisionReport>,
}

pub(crate) fn run_compiler_deferred_resolution(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    opts: CompilerDeferredResolutionOptions<'_>,
) -> Result<CompilerDeferredResolutionRun, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(opts.report_path)?;
    let input: CompilerRunReportInput = serde_json::from_str(&raw)?;
    let page_index = PageLookup::new(&eng.store.pages, opts.scope);
    let mut decisions = Vec::new();

    for item in &input.deferred_resolutions {
        decisions.push(classify_deferred_item(item, &page_index, opts.allow_create));
    }

    let mut aliases = Vec::new();
    let mut created_pages = Vec::new();
    if opts.apply {
        for decision in &mut decisions {
            match decision.decision {
                DeferredDecisionKind::AliasExisting => {
                    let Some(page_id) = decision.canonical_page_id else {
                        decision.applied = false;
                        continue;
                    };
                    let Some(page) = eng.store.pages.get(&page_id) else {
                        decision.applied = false;
                        continue;
                    };
                    if let Some(mapping) = alias_mapping_for_decision(
                        &decision.draft_title,
                        page,
                        opts.scope,
                        "compiler-deferred-resolution",
                    ) {
                        aliases.push(mapping);
                        decision.applied = true;
                    }
                }
                DeferredDecisionKind::CreateCanonical => {
                    let entry_type = parse_entry_type(&decision.draft_entry_type);
                    let page = build_deferred_page(
                        &decision.draft_title,
                        entry_type,
                        opts.scope,
                        opts.schema,
                        decision.source_title.as_deref(),
                        decision.source_url.as_deref(),
                    );
                    decision.canonical_page_id = Some(page.id);
                    decision.canonical_title = Some(page.title.clone());
                    decision.applied = true;
                    created_pages.push(page);
                }
                DeferredDecisionKind::IgnoreNoise | DeferredDecisionKind::KeepDeferred => {}
            }
        }
    }

    let mut changed_page_ids = Vec::new();
    if opts.apply && (!aliases.is_empty() || !created_pages.is_empty()) {
        for page in created_pages {
            changed_page_ids.push(page.id);
            eng.write_page(page, "compiler-deferred-resolution");
        }
        let snapshot = eng.store.to_snapshot(&eng.audits);
        repo.save_snapshot_and_append_outbox_with_aliases(&snapshot, &eng.outbox, &aliases)?;
        eng.outbox.clear();
    }

    let aliases_applied = if opts.apply { aliases.len() } else { 0 };
    let pages_created = changed_page_ids.len();
    let (json_report_path, markdown_report_path) = write_audit_report(
        opts.report_dir,
        opts.report_path,
        opts.apply,
        opts.allow_create,
        aliases_applied,
        pages_created,
        &decisions,
    )?;

    Ok(CompilerDeferredResolutionRun {
        db_changed: aliases_applied > 0 || pages_created > 0,
        aliases_applied,
        pages_created,
        changed_page_ids,
        json_report_path,
        markdown_report_path,
    })
}

fn classify_deferred_item(
    item: &DeferredResolutionInput,
    page_index: &PageLookup<'_>,
    allow_create: bool,
) -> DeferredDecisionReport {
    let entry_type = parse_entry_type(&item.draft_entry_type);
    let candidates = resolve_candidates(item, page_index);
    let base = |decision, reason: String, confidence: &str| DeferredDecisionReport {
        draft_title: item.draft_title.clone(),
        draft_entry_type: entry_type_label(&entry_type).to_string(),
        decision,
        reason,
        confidence: confidence.to_string(),
        source_title: item.source_title.clone(),
        source_url: item.source_url.clone(),
        canonical_page_id: None,
        canonical_title: None,
        candidates_seen: candidates.len(),
        applied: false,
    };

    if is_noise_title(&item.draft_title) {
        return base(
            DeferredDecisionKind::IgnoreNoise,
            "noise or generic title".to_string(),
            "high",
        );
    }

    if candidates.is_empty() {
        if allow_create {
            return base(
                DeferredDecisionKind::CreateCanonical,
                "no candidates and allow_create is enabled".to_string(),
                "medium",
            );
        }
        return base(
            DeferredDecisionKind::KeepDeferred,
            "no candidates; create disabled".to_string(),
            "low",
        );
    }

    if candidates.len() == 1 && is_safe_alias_candidate(&candidates[0]) {
        let candidate = &candidates[0];
        let mut decision = base(
            DeferredDecisionKind::AliasExisting,
            safe_alias_reason(candidate, &item.reason),
            "high",
        );
        decision.canonical_page_id = Some(candidate.page_id);
        decision.canonical_title = Some(candidate.title.clone());
        return decision;
    }

    base(
        DeferredDecisionKind::KeepDeferred,
        "ambiguous or low-confidence candidates remain machine-deferred".to_string(),
        "low",
    )
}

fn resolve_candidates(
    item: &DeferredResolutionInput,
    page_index: &PageLookup<'_>,
) -> Vec<ResolvedDeferredCandidate> {
    let mut out = Vec::new();
    for candidate in &item.candidates {
        if let Some(page_id) = candidate.page_id {
            if let Some(page) = page_index.by_id(page_id) {
                out.push(resolved_candidate_from_page(candidate, page));
                continue;
            }
        }
        if let Some(page) =
            page_index.unique_by_title_and_type(&candidate.title, &candidate.entry_type)
        {
            out.push(resolved_candidate_from_page(candidate, page));
        }
    }
    out
}

fn resolved_candidate_from_page(
    candidate: &DeferredResolutionCandidateInput,
    page: &WikiPage,
) -> ResolvedDeferredCandidate {
    ResolvedDeferredCandidate {
        page_id: page.id,
        title: page.title.clone(),
        score: candidate.score,
        match_reasons: candidate.match_reasons.clone(),
    }
}

fn is_safe_alias_candidate(candidate: &ResolvedDeferredCandidate) -> bool {
    candidate.score >= 90
        || candidate
            .match_reasons
            .iter()
            .any(|reason| reason == "exact key")
}

fn safe_alias_reason(candidate: &ResolvedDeferredCandidate, original_reason: &str) -> String {
    let mut reason = if original_reason.trim().is_empty() {
        "single safe candidate".to_string()
    } else {
        format!("single safe candidate after deferred: {original_reason}")
    };
    if !candidate.match_reasons.is_empty() {
        reason.push_str("; ");
        reason.push_str(&candidate.match_reasons.join(", "));
    }
    reason
}

struct PageLookup<'a> {
    by_id: HashMap<PageId, &'a WikiPage>,
    by_title_type: HashMap<(String, String), Vec<&'a WikiPage>>,
}

impl<'a> PageLookup<'a> {
    fn new(pages: &'a HashMap<PageId, WikiPage>, scope: &Scope) -> Self {
        let mut by_id = HashMap::new();
        let mut by_title_type: HashMap<(String, String), Vec<&WikiPage>> = HashMap::new();
        for page in pages.values() {
            if page.scope != *scope {
                continue;
            }
            let Some(entry_type) = page.entry_type.as_ref() else {
                continue;
            };
            if !matches!(entry_type, EntryType::Concept | EntryType::Entity) {
                continue;
            }
            by_id.insert(page.id, page);
            by_title_type
                .entry((
                    compact_key(&page.title),
                    entry_type_label(entry_type).to_string(),
                ))
                .or_default()
                .push(page);
        }
        Self {
            by_id,
            by_title_type,
        }
    }

    fn by_id(&self, page_id: PageId) -> Option<&'a WikiPage> {
        self.by_id.get(&page_id).copied()
    }

    fn unique_by_title_and_type(&self, title: &str, entry_type: &str) -> Option<&'a WikiPage> {
        let key = (
            compact_key(title),
            entry_type_label(&parse_entry_type(entry_type)).to_string(),
        );
        let pages = self.by_title_type.get(&key)?;
        if pages.len() == 1 {
            Some(pages[0])
        } else {
            None
        }
    }
}

fn build_deferred_page(
    title: &str,
    entry_type: EntryType,
    scope: &Scope,
    schema: &DomainSchema,
    source_title: Option<&str>,
    source_url: Option<&str>,
) -> WikiPage {
    let source = match (source_title, source_url) {
        (Some(title), Some(url)) if !url.trim().is_empty() => {
            format!("- {title} - {url}")
        }
        (Some(title), _) => format!("- {title}"),
        (_, Some(url)) if !url.trim().is_empty() => format!("- {url}"),
        _ => "- compiler deferred resolution".to_string(),
    };
    let status = initial_status_for(Some(&entry_type), schema);
    let mut page = PageContract::new(title, entry_type)
        .with_source("compiler-deferred-resolution")
        .with_section(
            "定义",
            "由 compiler deferred resolver 创建；后续 source 编译可补充定义。",
        )
        .with_section("关键要点", "（待后续 compiler 补充）")
        .with_section("本文语境", "（待后续 compiler 补充）")
        .with_section("来源引用", source)
        .into_page(scope.clone(), status);
    page.confidence = Confidence::Low;
    page.compiled_by = Some("compiler-deferred-resolution".to_string());
    page.last_compiled_at = Some(OffsetDateTime::now_utc());
    page
}

fn alias_mapping_for_decision(
    alias_text: &str,
    page: &WikiPage,
    scope: &Scope,
    source: &str,
) -> Option<CanonicalAliasMapping> {
    let alias_text = alias_text.trim();
    let normalized_alias_key = compact_key(alias_text);
    if alias_text.is_empty()
        || normalized_alias_key.is_empty()
        || normalized_alias_key == compact_key(&page.title)
    {
        return None;
    }
    let now = OffsetDateTime::now_utc();
    Some(CanonicalAliasMapping {
        alias_text: alias_text.to_string(),
        normalized_alias_key,
        canonical_page_id: page.id,
        canonical_title: page.title.clone(),
        entry_type: page.entry_type.clone()?,
        scope: scope.clone(),
        source: source.to_string(),
        confidence: 0.95,
        created_at: now,
        updated_at: now,
    })
}

fn write_audit_report(
    report_dir: &Path,
    input_report: &Path,
    apply: bool,
    allow_create: bool,
    aliases_applied: usize,
    pages_created: usize,
    decisions: &[DeferredDecisionReport],
) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let at = OffsetDateTime::now_utc().format(&Rfc3339)?;
    let slug = filename_timestamp(&at);
    let json_path = report_dir.join(format!("compiler-deferred-resolution-{slug}.json"));
    let markdown_path = report_dir.join(format!("compiler-deferred-resolution-{slug}.md"));
    let mut counts = BTreeMap::new();
    for decision in decisions {
        *counts
            .entry(decision_kind_label(&decision.decision).to_string())
            .or_insert(0) += 1;
    }
    let report = DeferredResolutionAuditReport {
        version: 1,
        kind: "compiler_deferred_resolution",
        at,
        mode: if apply { "apply" } else { "dry-run" },
        input_report: input_report.display().to_string(),
        allow_create,
        counts,
        aliases_applied,
        pages_created,
        decisions: decisions.to_vec(),
    };
    std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;
    std::fs::write(&markdown_path, render_markdown_report(&report))?;
    Ok((json_path, markdown_path))
}

fn render_markdown_report(report: &DeferredResolutionAuditReport) -> String {
    let mut out = format!(
        "# Compiler Deferred Resolution\n\n- at: `{}`\n- mode: `{}`\n- input_report: `{}`\n- aliases_applied: `{}`\n- pages_created: `{}`\n\n## Decisions\n\n",
        report.at,
        report.mode,
        report.input_report,
        report.aliases_applied,
        report.pages_created
    );
    for decision in &report.decisions {
        out.push_str(&format!(
            "- `{}` {} -> {}: {}",
            decision.draft_entry_type,
            decision.draft_title,
            decision_kind_label(&decision.decision),
            decision.reason
        ));
        if let Some(title) = &decision.canonical_title {
            out.push_str(&format!(" ({title})"));
        }
        out.push('\n');
    }
    out
}

fn decision_kind_label(decision: &DeferredDecisionKind) -> &'static str {
    match decision {
        DeferredDecisionKind::AliasExisting => "alias_existing",
        DeferredDecisionKind::CreateCanonical => "create_canonical",
        DeferredDecisionKind::IgnoreNoise => "ignore_noise",
        DeferredDecisionKind::KeepDeferred => "keep_deferred",
    }
}

fn filename_timestamp(at: &str) -> String {
    at.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn parse_entry_type(raw: &str) -> EntryType {
    match raw.trim().to_ascii_lowercase().as_str() {
        "concept" => EntryType::Concept,
        "entity" => EntryType::Entity,
        _ => EntryType::Concept,
    }
}

fn entry_type_label(entry_type: &EntryType) -> &'static str {
    match entry_type {
        EntryType::Concept => "concept",
        EntryType::Entity => "entity",
        EntryType::Summary => "summary",
        EntryType::Synthesis => "synthesis",
        EntryType::Qa => "qa",
        EntryType::LintReport => "lint_report",
        EntryType::Index => "index",
    }
}

fn is_noise_title(title: &str) -> bool {
    matches!(
        compact_key(title).as_str(),
        "" | "ai"
            | "人工智能"
            | "效率"
            | "技术"
            | "产品"
            | "工具"
            | "系统"
            | "其他"
            | "unknown"
            | "n/a"
            | "na"
    )
}

fn compact_key(title: &str) -> String {
    title
        .trim()
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch.is_alphanumeric() {
                Some(ch)
            } else if ch.is_whitespace() || ch.is_ascii_punctuation() || is_cjk_punctuation(ch) {
                None
            } else {
                Some(ch)
            }
        })
        .collect()
}

fn is_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '、'
            | '：'
            | '；'
            | '？'
            | '！'
            | '（'
            | '）'
            | '【'
            | '】'
            | '《'
            | '》'
            | '“'
            | '”'
            | '‘'
            | '’'
            | '「'
            | '」'
            | '『'
            | '』'
            | '·'
            | '—'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use wiki_core::{EntryStatus, Scope};
    use wiki_storage::WikiRepository;

    fn test_scope() -> Scope {
        Scope::Shared {
            team_id: "wiki".to_string(),
        }
    }

    fn write_report(
        dir: &Path,
        draft_title: &str,
        candidate_page: Option<&WikiPage>,
        extra_candidate: Option<&WikiPage>,
    ) -> PathBuf {
        let candidates: Vec<_> = candidate_page
            .into_iter()
            .chain(extra_candidate)
            .map(|page| {
                serde_json::json!({
                    "page_id": page.id,
                    "title": page.title,
                    "entry_type": entry_type_label(page.entry_type.as_ref().unwrap()),
                    "score": 100,
                    "match_reasons": ["exact key"]
                })
            })
            .collect();
        let path = dir.join("compiler-run.json");
        let json = serde_json::json!({
            "version": 1,
            "kind": "production_wiki_compiler_run",
            "deferred_resolutions": [{
                "source_title": "Source X",
                "source_url": "https://example.test/source",
                "draft_title": draft_title,
                "draft_entry_type": "concept",
                "reason": "ambiguous candidates",
                "candidates": candidates,
                "owner": "resolver-lint-fixer",
                "next_step": "machine_resolution"
            }]
        });
        std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
        path
    }

    fn existing_page(title: &str) -> WikiPage {
        WikiPage::new(title, format!("# {title}\n"), test_scope())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Draft)
    }

    #[test]
    fn compiler_resolve_deferred_aliases_single_exact_candidate() {
        let dir = tempdir().unwrap();
        let repo = SqliteRepository::open(dir.path().join("wiki.db")).unwrap();
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let page = existing_page("MCP 协议");
        eng.store.pages.insert(page.id, page.clone());
        repo.save_snapshot(&eng.store.to_snapshot(&eng.audits))
            .unwrap();
        let report_path = write_report(dir.path(), "MCP connectors", Some(&page), None);

        let run = run_compiler_deferred_resolution(
            &mut eng,
            &repo,
            CompilerDeferredResolutionOptions {
                report_path: &report_path,
                report_dir: dir.path(),
                apply: true,
                allow_create: false,
                scope: &test_scope(),
                schema: &DomainSchema::permissive_default(),
            },
        )
        .unwrap();

        assert!(run.db_changed);
        assert_eq!(run.aliases_applied, 1);
        let alias = repo
            .find_canonical_alias(&test_scope(), &compact_key("MCP connectors"))
            .unwrap()
            .unwrap();
        assert_eq!(alias.canonical_page_id, page.id);
    }

    #[test]
    fn compiler_resolve_deferred_keeps_multi_candidate_ambiguous() {
        let dir = tempdir().unwrap();
        let repo = SqliteRepository::open(dir.path().join("wiki.db")).unwrap();
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let page_a = existing_page("MCP 协议");
        let page_b = existing_page("MCP 连接器");
        eng.store.pages.insert(page_a.id, page_a.clone());
        eng.store.pages.insert(page_b.id, page_b.clone());
        repo.save_snapshot(&eng.store.to_snapshot(&eng.audits))
            .unwrap();
        let report_path = write_report(dir.path(), "MCP connectors", Some(&page_a), Some(&page_b));

        let run = run_compiler_deferred_resolution(
            &mut eng,
            &repo,
            CompilerDeferredResolutionOptions {
                report_path: &report_path,
                report_dir: dir.path(),
                apply: true,
                allow_create: false,
                scope: &test_scope(),
                schema: &DomainSchema::permissive_default(),
            },
        )
        .unwrap();

        assert!(!run.db_changed);
        assert_eq!(run.aliases_applied, 0);
        let audit = std::fs::read_to_string(run.json_report_path).unwrap();
        assert!(audit.contains("keep_deferred"));
    }

    #[test]
    fn compiler_resolve_deferred_ignores_generic_noise() {
        let dir = tempdir().unwrap();
        let repo = SqliteRepository::open(dir.path().join("wiki.db")).unwrap();
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        repo.save_snapshot(&eng.store.to_snapshot(&eng.audits))
            .unwrap();
        let report_path = write_report(dir.path(), "AI", None, None);

        let run = run_compiler_deferred_resolution(
            &mut eng,
            &repo,
            CompilerDeferredResolutionOptions {
                report_path: &report_path,
                report_dir: dir.path(),
                apply: true,
                allow_create: true,
                scope: &test_scope(),
                schema: &DomainSchema::permissive_default(),
            },
        )
        .unwrap();

        assert!(!run.db_changed);
        let audit = std::fs::read_to_string(run.json_report_path).unwrap();
        assert!(audit.contains("ignore_noise"));
    }

    #[test]
    fn compiler_resolve_deferred_can_create_when_allowed_and_no_candidates() {
        let dir = tempdir().unwrap();
        let repo = SqliteRepository::open(dir.path().join("wiki.db")).unwrap();
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        repo.save_snapshot(&eng.store.to_snapshot(&eng.audits))
            .unwrap();
        let report_path = write_report(dir.path(), "Agent Memory Boundary", None, None);

        let run = run_compiler_deferred_resolution(
            &mut eng,
            &repo,
            CompilerDeferredResolutionOptions {
                report_path: &report_path,
                report_dir: dir.path(),
                apply: true,
                allow_create: true,
                scope: &test_scope(),
                schema: &DomainSchema::permissive_default(),
            },
        )
        .unwrap();

        assert!(run.db_changed);
        assert_eq!(run.pages_created, 1);
        assert_eq!(run.changed_page_ids.len(), 1);
        assert!(eng
            .store
            .pages
            .values()
            .any(|page| page.title == "Agent Memory Boundary"));
        let created = eng
            .store
            .pages
            .values()
            .find(|page| page.title == "Agent Memory Boundary")
            .unwrap();
        assert!(!created.markdown.contains("[[摘要：Source X]]"));
    }
}
