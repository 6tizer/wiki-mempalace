use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use walkdir::WalkDir;
use wiki_core::{extract_wikilinks, AuditRecord, EntryType, RawArtifact, WikiPage};
use wiki_kernel::{write_projection, InMemoryStore};
use wiki_mempalace_bridge::{LiveMempalaceSink, MempalaceWikiSink};
use wiki_storage::WikiRepository;

const VERSION: u32 = 1;
const SOURCE_DRAWER_POLICY_NOTE: &str =
    "source drawers are out of scope; source bodies are not expected in Mempalace";

type DynError = Box<dyn std::error::Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbPageEvidence {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub entry_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbSourceEvidence {
    pub id: String,
    pub uri: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyAuditReport {
    pub version: u32,
    pub generated_at: String,
    pub db_path: String,
    pub wiki_dir: String,
    pub palace_path: Option<String>,
    pub db: DbAuditSection,
    pub vault: VaultAuditSection,
    pub palace: PalaceAuditSection,
    pub candidates: CandidateSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DbAuditSection {
    pub page_count: usize,
    pub source_count: usize,
    pub page_ids: Vec<String>,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultAuditSection {
    pub scanned_roots: Vec<String>,
    pub page_files: usize,
    pub source_files: usize,
    pub managed_files: Vec<String>,
    pub missing_pages: Vec<String>,
    pub missing_sources: Vec<String>,
    pub extra_pages: Vec<String>,
    pub extra_sources: Vec<String>,
    pub empty_unmanaged_files: Vec<String>,
    pub stale_notion_links: Vec<StaleNotionLinkCandidate>,
    pub unresolved_local_links: Vec<UnresolvedLocalLink>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PalaceAuditSection {
    pub skipped: bool,
    pub drawer_count: usize,
    pub page_drawer_count: usize,
    pub missing_page_drawers: Vec<String>,
    pub stale_page_drawers: Vec<String>,
    pub stale_page_drawer_contents: Vec<String>,
    pub source_drawer_policy_note: String,
    pub warnings: Vec<String>,
}

impl Default for PalaceAuditSection {
    fn default() -> Self {
        Self {
            skipped: true,
            drawer_count: 0,
            page_drawer_count: 0,
            missing_page_drawers: Vec::new(),
            stale_page_drawers: Vec::new(),
            stale_page_drawer_contents: Vec::new(),
            source_drawer_policy_note: SOURCE_DRAWER_POLICY_NOTE.to_string(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CandidateSection {
    pub source_summary_exact_matches: Vec<SourceSummaryCandidate>,
    pub source_summary_needs_human: Vec<SourceSummaryCandidate>,
    pub safe_cleanup_candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyAuditReportFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaleNotionLinkCandidate {
    pub page_id: String,
    pub page_title: String,
    pub raw_target: String,
    pub decoded_target: String,
    pub reason: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_target_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnresolvedLocalLink {
    pub page_id: String,
    pub page_title: String,
    pub target: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SourceSummaryCandidateKind {
    ExactTitle,
    ExactUrl,
    NeedsHuman,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSummaryCandidate {
    pub source_id: String,
    pub source_uri: String,
    pub source_title: Option<String>,
    pub summary_page_id: String,
    pub summary_title: String,
    pub kind: SourceSummaryCandidateKind,
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct ConsistencyAuditOptions {
    pub db_path: PathBuf,
    pub wiki_dir: PathBuf,
    pub palace_path: Option<PathBuf>,
    pub generated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyPlan {
    pub version: u32,
    pub generated_at: String,
    pub audit_report_path: String,
    pub wiki_dir: String,
    pub actions: Vec<ConsistencyPlanAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsistencyPlanAction {
    pub kind: ConsistencyActionKind,
    pub path: String,
    pub operation: String,
    pub value: Option<String>,
    pub reason: String,
    pub executable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyActionKind {
    DbFix,
    VaultCleanup,
    PalaceReplay,
    NeedsHuman,
    Deferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyPlanFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyApplyReport {
    pub mode: String,
    pub actions_seen: usize,
    pub executable_actions: usize,
    pub db_fixes_planned: usize,
    pub db_fixes_applied: usize,
    pub vault_cleanups_planned: usize,
    pub vault_cleanups_applied: usize,
    pub palace_replays_planned: usize,
    pub palace_replays_applied: usize,
    pub projection_ran: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ConsistencyApplyOptions<'a> {
    pub plan_path: &'a Path,
    pub wiki_dir: &'a Path,
    pub palace_path: Option<&'a Path>,
    pub palace_bank_id: &'a str,
    pub apply: bool,
}

pub fn run_consistency_audit<R: WikiRepository>(
    repo: &R,
    options: ConsistencyAuditOptions,
) -> Result<(ConsistencyAuditReport, ConsistencyAuditReportFiles), DynError> {
    if !options.wiki_dir.is_dir() {
        return Err(format!("wiki-dir 不存在: {}", options.wiki_dir.display()).into());
    }
    let snapshot = repo.load_snapshot()?;
    let pages = db_page_evidence(&snapshot.pages);
    let sources = db_source_evidence(&snapshot.sources);
    let vault = scan_vault(&options.wiki_dir, &pages, &sources)?;
    let palace = match &options.palace_path {
        Some(path) => audit_palace(path, &pages),
        None => PalaceAuditSection::default(),
    };
    let candidates = collect_candidates(&sources, &pages, &vault.empty_unmanaged_files);
    let report = ConsistencyAuditReport {
        version: VERSION,
        generated_at: options.generated_at.format(&Rfc3339)?,
        db_path: options.db_path.display().to_string(),
        wiki_dir: options.wiki_dir.display().to_string(),
        palace_path: options.palace_path.map(|path| path.display().to_string()),
        db: DbAuditSection {
            page_count: pages.len(),
            source_count: sources.len(),
            page_ids: pages.iter().map(|page| page.id.clone()).collect(),
            source_ids: sources.iter().map(|source| source.id.clone()).collect(),
        },
        vault,
        palace,
        candidates,
    };
    let files = write_json_and_markdown(&report, options.wiki_dir.join("reports"))?;
    Ok((report, files))
}

pub fn run_consistency_plan(
    audit_report: &Path,
    report_dir: Option<PathBuf>,
    generated_at: OffsetDateTime,
) -> Result<(ConsistencyPlan, ConsistencyPlanFiles), DynError> {
    ensure_timestamped_audit_path(audit_report)?;
    let body = fs::read_to_string(audit_report)?;
    let audit: ConsistencyAuditReport = serde_json::from_str(&body)?;
    let mut actions = Vec::new();

    for path in &audit.candidates.safe_cleanup_candidates {
        actions.push(ConsistencyPlanAction {
            kind: ConsistencyActionKind::VaultCleanup,
            path: path.clone(),
            operation: "delete_unmanaged_empty_file".to_string(),
            value: None,
            reason: "Vault 文件为空，且 audit 未发现 DB 管理身份。".to_string(),
            executable: true,
        });
    }
    for candidate in &audit.candidates.source_summary_exact_matches {
        actions.push(ConsistencyPlanAction {
            kind: ConsistencyActionKind::DbFix,
            path: format!("wiki://page/{}", candidate.summary_page_id),
            operation: "append_source_reference".to_string(),
            value: Some(candidate.source_uri.clone()),
            reason: "source 与 summary 有精确 URL/title 证据，可在 DB page 中补来源引用。"
                .to_string(),
            executable: true,
        });
    }
    for stale in &audit.vault.stale_notion_links {
        let report_only = stale.reason == "notion_url";
        let retired_system_page = is_retired_notion_system_page(&stale.decoded_target);
        let resolved = stale.resolved_target_path.is_some()
            && stale.replacement_target.is_some()
            && stale.resolution.as_deref() == Some("notion_uuid_page");
        if resolved {
            actions.push(ConsistencyPlanAction {
                kind: ConsistencyActionKind::DbFix,
                path: format!("wiki://page/{}", stale.page_id),
                operation: "replace_legacy_notion_link".to_string(),
                value: Some(format!(
                    "{}\t{}",
                    stale.raw_target,
                    stale.replacement_target.clone().unwrap_or_default()
                )),
                reason: "旧 Notion 导出链接已通过 notion_uuid 对上当前页面，可在 DB page 中自动改成当前 Vault 链接。"
                    .to_string(),
                executable: true,
            });
            continue;
        }
        actions.push(ConsistencyPlanAction {
            kind: if report_only || retired_system_page {
                ConsistencyActionKind::Deferred
            } else {
                ConsistencyActionKind::NeedsHuman
            },
            path: format!("wiki://page/{}", stale.page_id),
            operation: if retired_system_page {
                "record_retired_notion_system_link"
            } else if report_only {
                "record_legacy_notion_url"
            } else {
                "review_stale_notion_link"
            }
            .to_string(),
            value: Some(stale.raw_target.clone()),
            reason: if retired_system_page {
                "旧 Notion 系统页已确认不进入本地 active wiki，本轮只报告，不要求人工逐条处理。"
            } else if report_only {
                "旧 Notion URL 是来源脚印，本轮只报告，不要求人工逐条处理。"
            } else {
                "旧 Notion 导出文件名意图不能安全判断，先保留人工复核项。"
            }
            .to_string(),
            executable: false,
        });
    }
    for path in &audit.palace.missing_page_drawers {
        actions.push(ConsistencyPlanAction {
            kind: ConsistencyActionKind::PalaceReplay,
            path: path.clone(),
            operation: "replay_page_to_mempalace".to_string(),
            value: None,
            reason: "DB page 缺少 Mempalace page drawer，走 page sink 重放。".to_string(),
            executable: true,
        });
    }
    for candidate in &audit.candidates.source_summary_needs_human {
        actions.push(ConsistencyPlanAction {
            kind: ConsistencyActionKind::Deferred,
            path: format!("wiki://page/{}", candidate.summary_page_id),
            operation: "review_source_summary_candidate".to_string(),
            value: Some(candidate.source_uri.clone()),
            reason: "source 与 summary 只有近似证据，不能自动写。".to_string(),
            executable: false,
        });
    }

    let plan = ConsistencyPlan {
        version: VERSION,
        generated_at: generated_at.format(&Rfc3339)?,
        audit_report_path: audit_report.display().to_string(),
        wiki_dir: audit.wiki_dir.clone(),
        actions,
    };
    validate_plan(&plan)?;
    validate_plan_against_audit(&plan, &audit)?;
    let dir = report_dir.unwrap_or_else(|| PathBuf::from(&audit.wiki_dir).join("reports"));
    let files = write_plan_json_and_markdown(&plan, dir)?;
    Ok((plan, files))
}

pub fn write_plan_json_and_markdown(
    plan: &ConsistencyPlan,
    report_dir: impl AsRef<Path>,
) -> Result<ConsistencyPlanFiles, DynError> {
    let report_dir = report_dir.as_ref();
    fs::create_dir_all(report_dir)?;
    let stem = format!(
        "consistency-plan-{}",
        filename_timestamp(&plan.generated_at)
    );
    let json_path = report_dir.join(format!("{stem}.json"));
    let markdown_path = report_dir.join(format!("{stem}.md"));
    fs::write(&json_path, serde_json::to_string_pretty(plan)?)?;
    fs::write(
        &markdown_path,
        render_plan_markdown(
            plan,
            json_path.file_name().unwrap().to_string_lossy().as_ref(),
        ),
    )?;
    Ok(ConsistencyPlanFiles {
        json_path,
        markdown_path,
    })
}

pub fn render_plan_markdown(plan: &ConsistencyPlan, sibling_json: &str) -> String {
    let executable = plan
        .actions
        .iter()
        .filter(|action| action.executable)
        .count();
    let mut md = format!(
        "# 一致性治理计划\n\n- generated_at: `{}`\n- audit_report: `{}`\n- source_of_truth: `{}`\n- actions: `{}`\n- executable_actions: `{}`\n\n",
        plan.generated_at,
        plan.audit_report_path,
        sibling_json,
        plan.actions.len(),
        executable
    );
    md.push_str("## 动作\n\n");
    for action in &plan.actions {
        md.push_str(&format!(
            "- `{}` `{}` `{}` executable={}：{}\n",
            action_kind_str(action.kind),
            action.operation,
            action.path,
            action.executable,
            action.reason
        ));
    }
    md
}

pub fn run_consistency_apply<R: WikiRepository>(
    repo: &R,
    store: &mut InMemoryStore,
    audits: &[AuditRecord],
    options: ConsistencyApplyOptions<'_>,
) -> Result<ConsistencyApplyReport, DynError> {
    let body = fs::read_to_string(options.plan_path)?;
    let plan: ConsistencyPlan = serde_json::from_str(&body)?;
    validate_plan(&plan)?;
    let audit_body = fs::read_to_string(&plan.audit_report_path)?;
    let audit: ConsistencyAuditReport = serde_json::from_str(&audit_body)?;
    validate_apply_context(&plan, &audit, options.wiki_dir)?;
    validate_plan_against_audit(&plan, &audit)?;

    let executable_actions = plan
        .actions
        .iter()
        .filter(|action| action.executable)
        .count();
    let db_fixes_planned = plan
        .actions
        .iter()
        .filter(|action| action.kind == ConsistencyActionKind::DbFix && action.executable)
        .count();
    let vault_cleanups_planned = plan
        .actions
        .iter()
        .filter(|action| action.kind == ConsistencyActionKind::VaultCleanup && action.executable)
        .count();
    let palace_replays_planned = plan
        .actions
        .iter()
        .filter(|action| action.kind == ConsistencyActionKind::PalaceReplay && action.executable)
        .count();

    let mut report = ConsistencyApplyReport {
        mode: if options.apply { "apply" } else { "dry-run" }.to_string(),
        actions_seen: plan.actions.len(),
        executable_actions,
        db_fixes_planned,
        db_fixes_applied: 0,
        vault_cleanups_planned,
        vault_cleanups_applied: 0,
        palace_replays_planned,
        palace_replays_applied: 0,
        projection_ran: false,
    };
    if !options.apply {
        return Ok(report);
    }

    let db_fix_page_ids: BTreeSet<_> = plan
        .actions
        .iter()
        .filter(|action| action.executable && action.kind == ConsistencyActionKind::DbFix)
        .filter_map(|action| action.path.strip_prefix("wiki://page/"))
        .map(str::to_string)
        .collect();
    let mut db_changed = false;
    let mut db_changed_page_ids = BTreeSet::new();
    for action in plan
        .actions
        .iter()
        .filter(|action| action.executable && action.kind == ConsistencyActionKind::DbFix)
    {
        if apply_db_fix(store, action)? {
            report.db_fixes_applied += 1;
            db_changed = true;
            if let Some(page_id) = action.path.strip_prefix("wiki://page/") {
                db_changed_page_ids.insert(page_id.to_string());
            }
        }
    }
    if db_changed {
        repo.save_snapshot(&store.to_snapshot(audits))?;
    }
    if !db_fix_page_ids.is_empty() {
        project_db_fixed_pages_to_vault(options.wiki_dir, store, &db_fix_page_ids)?;
        report.projection_ran = true;
    } else if db_changed {
        write_projection(options.wiki_dir, store, audits)?;
        report.projection_ran = true;
    }

    let known_paths = db_known_projection_paths(
        options.wiki_dir,
        &db_page_evidence(&store.pages.values().cloned().collect::<Vec<_>>()),
        &db_source_evidence(&store.sources.values().cloned().collect::<Vec<_>>()),
    );
    for action in plan
        .actions
        .iter()
        .filter(|action| action.executable && action.kind == ConsistencyActionKind::VaultCleanup)
    {
        if known_paths.contains(&action.path) {
            return Err(format!(
                "refusing to delete DB-known projection path: {}",
                action.path
            )
            .into());
        }
        if apply_vault_cleanup(options.wiki_dir, action)? {
            report.vault_cleanups_applied += 1;
        }
    }

    let mut palace_replay_page_ids = db_changed_page_ids;
    palace_replay_page_ids.extend(db_fix_page_ids);
    for action in plan
        .actions
        .iter()
        .filter(|action| action.executable && action.kind == ConsistencyActionKind::PalaceReplay)
    {
        let page_id = action.path.strip_prefix("wiki://page/").ok_or_else(|| {
            format!(
                "palace replay path must be wiki://page/<id>: {}",
                action.path
            )
        })?;
        palace_replay_page_ids.insert(page_id.to_string());
    }

    if !palace_replay_page_ids.is_empty() {
        let palace_path = options
            .palace_path
            .ok_or("--palace is required when DB fixes or palace replay need Mempalace sync")?;
        let sink = LiveMempalaceSink::open(palace_path, options.palace_bank_id)?;
        for page_id in palace_replay_page_ids {
            let page = store
                .pages
                .values()
                .find(|page| page.id.0.to_string() == page_id.as_str())
                .ok_or_else(|| format!("page not found for palace replay: {page_id}"))?;
            sink.on_page_written(page)?;
            report.palace_replays_applied += 1;
        }
    }

    Ok(report)
}

pub fn write_json_and_markdown(
    report: &ConsistencyAuditReport,
    report_dir: impl AsRef<Path>,
) -> Result<ConsistencyAuditReportFiles, DynError> {
    let report_dir = report_dir.as_ref();
    fs::create_dir_all(report_dir)?;
    let stem = format!(
        "consistency-audit-{}",
        filename_timestamp(&report.generated_at)
    );
    let json_path = report_dir.join(format!("{stem}.json"));
    let markdown_path = report_dir.join(format!("{stem}.md"));
    fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    fs::write(
        &markdown_path,
        render_markdown(
            report,
            json_path.file_name().unwrap().to_string_lossy().as_ref(),
        ),
    )?;
    Ok(ConsistencyAuditReportFiles {
        json_path,
        markdown_path,
    })
}

pub fn render_markdown(report: &ConsistencyAuditReport, sibling_json: &str) -> String {
    format!(
        concat!(
            "# 一致性审计\n\n",
            "- generated_at: `{}`\n",
            "- db_path: `{}`\n",
            "- wiki_dir: `{}`\n",
            "- palace_path: `{}`\n\n",
            "> 同名 JSON 是机器事实源：`{}`。\n\n",
            "## DB\n\n",
            "- pages: {}\n",
            "- sources: {}\n\n",
            "## Vault\n\n",
            "- scanned_roots: pages/, sources/\n",
            "- page_files: {}\n",
            "- source_files: {}\n",
            "- missing_pages: {}\n",
            "- missing_sources: {}\n",
            "- extra_pages: {}\n",
            "- extra_sources: {}\n",
            "- empty_unmanaged_files: {}\n",
            "- stale_notion_links: {}\n",
            "- unresolved_local_links: {}\n\n",
            "## Mempalace\n\n",
            "- skipped: {}\n",
            "- drawers: {}\n",
            "- page_drawers: {}\n",
            "- missing_page_drawers: {}\n",
            "- stale_page_drawers: {}\n",
            "- stale_page_drawer_contents: {}\n",
            "- source_drawer_policy: {}\n\n",
            "## 候选项\n\n",
            "- source_summary_exact_matches: {}\n",
            "- source_summary_needs_human: {}\n",
            "- safe_cleanup_candidates: {}\n"
        ),
        report.generated_at,
        report.db_path,
        report.wiki_dir,
        report.palace_path.as_deref().unwrap_or("skipped"),
        sibling_json,
        report.db.page_count,
        report.db.source_count,
        report.vault.page_files,
        report.vault.source_files,
        report.vault.missing_pages.len(),
        report.vault.missing_sources.len(),
        report.vault.extra_pages.len(),
        report.vault.extra_sources.len(),
        report.vault.empty_unmanaged_files.len(),
        report.vault.stale_notion_links.len(),
        report.vault.unresolved_local_links.len(),
        report.palace.skipped,
        report.palace.drawer_count,
        report.palace.page_drawer_count,
        report.palace.missing_page_drawers.len(),
        report.palace.stale_page_drawers.len(),
        report.palace.stale_page_drawer_contents.len(),
        report.palace.source_drawer_policy_note,
        report.candidates.source_summary_exact_matches.len(),
        report.candidates.source_summary_needs_human.len(),
        report.candidates.safe_cleanup_candidates.len(),
    )
}

pub fn find_stale_notion_link_candidates(
    pages: &[DbPageEvidence],
    known_vault_paths: &BTreeSet<String>,
    notion_uuid_paths: &BTreeMap<String, String>,
) -> Vec<StaleNotionLinkCandidate> {
    let mut out = Vec::new();
    for page in pages {
        for raw_target in markdown_link_targets(&page.markdown) {
            let decoded = percent_decode(&raw_target);
            if local_target_exists(&raw_target, &decoded, known_vault_paths) {
                continue;
            }
            let mut reasons = Vec::new();
            if raw_target.contains('%') && decoded != raw_target {
                reasons.push("url_encoded");
            }
            if raw_target.contains("notion.so") || decoded.contains("notion.so") {
                reasons.push("notion_url");
            }
            if looks_like_notion_export_filename(&decoded) {
                reasons.push("notion_export_filename");
            }
            if reasons.is_empty() {
                continue;
            }
            let resolved_target_path = notion_uuid_from_target(&decoded)
                .and_then(|uuid| notion_uuid_paths.get(&uuid).cloned());
            let replacement_target = resolved_target_path
                .as_deref()
                .map(|path| relative_target_from_page(page.entry_type.as_deref(), path));
            let resolution = resolved_target_path
                .as_ref()
                .map(|_| "notion_uuid_page".to_string());
            out.push(StaleNotionLinkCandidate {
                page_id: page.id.clone(),
                page_title: page.title.clone(),
                raw_target,
                decoded_target: decoded,
                reason: reasons.join(","),
                action: "candidate_only".to_string(),
                resolved_target_path,
                replacement_target,
                resolution,
            });
        }
    }
    out
}

pub fn find_source_summary_candidates(
    sources: &[DbSourceEvidence],
    summaries: &[DbPageEvidence],
) -> Vec<SourceSummaryCandidate> {
    let mut out = Vec::new();
    for source in sources {
        let source_title = frontmatter_value(&source.body, "title");
        let mut best: Option<SourceSummaryCandidate> = None;
        for summary in summaries {
            let kind = if summary_exact_url(summary, &source.uri) {
                Some(SourceSummaryCandidateKind::ExactUrl)
            } else if source_title
                .as_deref()
                .is_some_and(|title| summary_exact_title(summary, title))
            {
                Some(SourceSummaryCandidateKind::ExactTitle)
            } else if let Some(title) = &source_title {
                let clean_summary = strip_summary_title_prefix(&summary.title);
                if normalized_title(clean_summary).contains(&normalized_title(title))
                    || normalized_title(title).contains(&normalized_title(clean_summary))
                {
                    Some(SourceSummaryCandidateKind::NeedsHuman)
                } else {
                    None
                }
            } else {
                None
            };
            let Some(kind) = kind else {
                continue;
            };
            let candidate = SourceSummaryCandidate {
                source_id: source.id.clone(),
                source_uri: source.uri.clone(),
                source_title: source_title.clone(),
                summary_page_id: summary.id.clone(),
                summary_title: summary.title.clone(),
                kind,
                action: if kind == SourceSummaryCandidateKind::NeedsHuman {
                    "deferred"
                } else {
                    "candidate_only"
                }
                .to_string(),
            };
            if best
                .as_ref()
                .is_none_or(|existing| candidate.kind < existing.kind)
            {
                best = Some(candidate);
            }
        }
        if let Some(candidate) = best {
            out.push(candidate);
        }
    }
    out.sort_by(|a, b| {
        a.source_id
            .cmp(&b.source_id)
            .then_with(|| a.summary_page_id.cmp(&b.summary_page_id))
    });
    out
}

fn summary_exact_url(summary: &DbPageEvidence, source_uri: &str) -> bool {
    ["source_url", "source_uri", "uri", "url"]
        .iter()
        .any(|key| frontmatter_value(&summary.markdown, key).as_deref() == Some(source_uri))
}

fn summary_exact_title(summary: &DbPageEvidence, source_title: &str) -> bool {
    ["source_title", "title"]
        .iter()
        .any(|key| frontmatter_value(&summary.markdown, key).as_deref() == Some(source_title))
        || strip_summary_title_prefix(&summary.title) == source_title
}

fn strip_summary_title_prefix(title: &str) -> &str {
    title
        .trim_start_matches("摘要：")
        .trim_start_matches("Summary: ")
        .trim()
}

fn db_page_evidence(pages: &[WikiPage]) -> Vec<DbPageEvidence> {
    pages
        .iter()
        .map(|page| DbPageEvidence {
            id: page.id.0.to_string(),
            title: page.title.clone(),
            markdown: page.markdown.clone(),
            entry_type: page
                .entry_type
                .as_ref()
                .map(entry_type_label)
                .map(str::to_string),
        })
        .collect()
}

fn entry_type_label(kind: &EntryType) -> &'static str {
    match kind {
        EntryType::Concept => "concept",
        EntryType::Entity => "entity",
        EntryType::Summary => "summary",
        EntryType::Synthesis => "synthesis",
        EntryType::Qa => "qa",
        EntryType::LintReport => "lint_report",
        EntryType::Index => "index",
    }
}

fn db_source_evidence(sources: &[RawArtifact]) -> Vec<DbSourceEvidence> {
    sources
        .iter()
        .map(|source| DbSourceEvidence {
            id: source.id.0.to_string(),
            uri: source.uri.clone(),
            body: source.body.clone(),
        })
        .collect()
}

fn scan_vault(
    wiki_dir: &Path,
    pages: &[DbPageEvidence],
    sources: &[DbSourceEvidence],
) -> Result<VaultAuditSection, DynError> {
    let mut section = VaultAuditSection {
        scanned_roots: vec!["pages".to_string(), "sources".to_string()],
        ..VaultAuditSection::default()
    };
    let db_page_ids: BTreeSet<_> = pages.iter().map(|page| page.id.clone()).collect();
    let db_source_ids: BTreeSet<_> = sources.iter().map(|source| source.id.clone()).collect();
    let db_known_rel_paths = db_known_projection_paths(wiki_dir, pages, sources);
    let page_titles: BTreeSet<_> = pages.iter().map(|page| page.title.clone()).collect();
    let mut vault_page_ids: BTreeMap<String, String> = BTreeMap::new();
    let mut vault_source_ids: BTreeMap<String, String> = BTreeMap::new();
    let mut known_vault_paths: BTreeSet<String> = BTreeSet::new();
    let mut notion_uuid_paths: BTreeMap<String, String> = BTreeMap::new();

    for root in ["pages", "sources"] {
        let root_path = wiki_dir.join(root);
        if !root_path.exists() {
            continue;
        }
        for entry in WalkDir::new(&root_path).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let rel = relative_slash_path(wiki_dir, path);
            if root == "pages" {
                section.page_files += 1;
            } else {
                section.source_files += 1;
            }
            known_vault_paths.insert(rel.clone());
            let bytes = match fs::read(path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    section.warnings.push(format!("read failed: {rel}: {err}"));
                    continue;
                }
            };
            let text = String::from_utf8_lossy(&bytes);
            let id_key = if root == "pages" {
                "page_id"
            } else {
                "source_id"
            };
            let id = frontmatter_value(&text, id_key).or_else(|| frontmatter_value(&text, "id"));
            if let Some(id) = id {
                section.managed_files.push(rel.clone());
                if root == "pages" {
                    vault_page_ids.insert(id, rel.clone());
                    if let Some(notion_uuid) = frontmatter_value(&text, "notion_uuid") {
                        notion_uuid_paths.insert(notion_uuid, rel.clone());
                    }
                } else {
                    vault_source_ids.insert(id, rel.clone());
                }
            } else if bytes.is_empty() && !db_known_rel_paths.contains(&rel) {
                section.empty_unmanaged_files.push(rel.clone());
            }
        }
    }

    section.missing_pages = db_page_ids
        .difference(&vault_page_ids.keys().cloned().collect())
        .map(|id| format!("wiki://page/{id}"))
        .collect();
    section.missing_sources = db_source_ids
        .difference(&vault_source_ids.keys().cloned().collect())
        .map(|id| format!("wiki://source/{id}"))
        .collect();
    section.extra_pages = vault_page_ids
        .keys()
        .filter(|id| !db_page_ids.contains(*id))
        .map(|id| vault_page_ids[id].clone())
        .collect();
    section.extra_sources = vault_source_ids
        .keys()
        .filter(|id| !db_source_ids.contains(*id))
        .map(|id| vault_source_ids[id].clone())
        .collect();
    section.stale_notion_links =
        find_stale_notion_link_candidates(pages, &known_vault_paths, &notion_uuid_paths);
    section.unresolved_local_links = find_unresolved_local_links(pages, &page_titles);
    section.managed_files.sort();
    section.empty_unmanaged_files.sort();
    Ok(section)
}

fn audit_palace(path: &Path, pages: &[DbPageEvidence]) -> PalaceAuditSection {
    let mut section = PalaceAuditSection {
        skipped: false,
        ..PalaceAuditSection::default()
    };
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(err) => {
            section.warnings.push(format!("open palace failed: {err}"));
            return section;
        }
    };
    section.drawer_count = query_count(&conn, "SELECT COUNT(*) FROM drawers", &mut section);
    let rows = match query_page_drawers(&conn, &mut section) {
        Some(rows) => rows,
        None => return section,
    };
    section.page_drawer_count = rows.len();
    let db_by_source_path: BTreeMap<String, String> = pages
        .iter()
        .filter(|page| is_palace_eligible_entry_type(page.entry_type.as_deref()))
        .map(|page| (format!("wiki://page/{}", page.id), page.markdown.clone()))
        .collect();
    let palace_by_source_path: BTreeMap<String, String> = rows.into_iter().collect();
    for source_path in db_by_source_path.keys() {
        if !palace_by_source_path.contains_key(source_path) {
            section.missing_page_drawers.push(source_path.clone());
        }
    }
    for (source_path, content) in &palace_by_source_path {
        match db_by_source_path.get(source_path) {
            Some(db_content) if db_content != content => {
                section.stale_page_drawer_contents.push(source_path.clone());
            }
            Some(_) => {}
            None => section.stale_page_drawers.push(source_path.clone()),
        }
    }
    section
}

fn is_palace_eligible_entry_type(entry_type: Option<&str>) -> bool {
    matches!(
        entry_type,
        Some("summary" | "concept" | "entity" | "synthesis" | "qa")
    )
}

fn query_count(conn: &Connection, sql: &str, section: &mut PalaceAuditSection) -> usize {
    match conn.query_row(sql, [], |row| row.get::<_, i64>(0)) {
        Ok(count) => count.max(0) as usize,
        Err(err) => {
            section.warnings.push(format!("palace count failed: {err}"));
            0
        }
    }
}

fn query_page_drawers(
    conn: &Connection,
    section: &mut PalaceAuditSection,
) -> Option<Vec<(String, String)>> {
    let mut stmt = match conn
        .prepare("SELECT source_path, content FROM drawers WHERE source_path LIKE 'wiki://page/%'")
    {
        Ok(stmt) => stmt,
        Err(err) => {
            section
                .warnings
                .push(format!("palace page drawer query failed: {err}"));
            return None;
        }
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(err) => {
            section
                .warnings
                .push(format!("palace page drawer read failed: {err}"));
            return None;
        }
    };
    let mut out = Vec::new();
    for row in rows {
        match row {
            Ok(value) => out.push(value),
            Err(err) => section
                .warnings
                .push(format!("palace page drawer row failed: {err}")),
        }
    }
    Some(out)
}

fn collect_candidates(
    sources: &[DbSourceEvidence],
    pages: &[DbPageEvidence],
    empty_unmanaged_files: &[String],
) -> CandidateSection {
    let summaries: Vec<_> = pages
        .iter()
        .filter(|page| {
            page.entry_type
                .as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("summary"))
                || frontmatter_value(&page.markdown, "entry_type")
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("summary"))
        })
        .cloned()
        .collect();
    let candidates = find_source_summary_candidates(sources, &summaries);
    CandidateSection {
        source_summary_exact_matches: candidates
            .iter()
            .filter(|candidate| candidate.kind != SourceSummaryCandidateKind::NeedsHuman)
            .cloned()
            .collect(),
        source_summary_needs_human: candidates
            .into_iter()
            .filter(|candidate| candidate.kind == SourceSummaryCandidateKind::NeedsHuman)
            .collect(),
        safe_cleanup_candidates: empty_unmanaged_files.to_vec(),
    }
}

fn ensure_timestamped_audit_path(path: &Path) -> Result<(), DynError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("audit report path must have a file name")?;
    if name == "consistency-audit.json" {
        return Err(
            "consistency-plan requires timestamped consistency-audit-<timestamp>.json".into(),
        );
    }
    if !name.starts_with("consistency-audit-") || !name.ends_with(".json") {
        return Err(format!(
            "consistency-plan requires timestamped consistency-audit-<timestamp>.json; got {name}"
        )
        .into());
    }
    Ok(())
}

fn validate_apply_context(
    plan: &ConsistencyPlan,
    audit: &ConsistencyAuditReport,
    wiki_dir: &Path,
) -> Result<(), DynError> {
    ensure_timestamped_audit_path(Path::new(&plan.audit_report_path))?;
    if plan.wiki_dir != audit.wiki_dir {
        return Err(format!(
            "plan wiki_dir does not match audit wiki_dir: {} != {}",
            plan.wiki_dir, audit.wiki_dir
        )
        .into());
    }
    if !same_path(&audit.wiki_dir, wiki_dir) {
        return Err(format!(
            "apply --wiki-dir does not match audit wiki_dir: {} != {}",
            wiki_dir.display(),
            audit.wiki_dir
        )
        .into());
    }
    Ok(())
}

fn same_path(left: &str, right: &Path) -> bool {
    let left_path = Path::new(left);
    match (left_path.canonicalize(), right.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => left_path == right,
    }
}

fn db_known_projection_paths(
    wiki_dir: &Path,
    pages: &[DbPageEvidence],
    sources: &[DbSourceEvidence],
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for page in pages {
        out.insert(format!(
            "pages/{}/{}.md",
            page_subdir(page.entry_type.as_deref()),
            page.title.replace('/', "-")
        ));
    }
    for source in sources {
        if let Some(rel) = source_relative_path(wiki_dir, &source.uri) {
            out.insert(rel);
        }
    }
    out
}

fn page_subdir(entry_type: Option<&str>) -> &'static str {
    match entry_type {
        Some("summary") => "summary",
        Some("concept") => "concept",
        Some("entity") => "entity",
        Some("synthesis") => "synthesis",
        Some("qa") => "qa",
        Some("lint_report") => "lint-report",
        Some("index") => "index",
        _ => "_unspecified",
    }
}

fn source_relative_path(wiki_dir: &Path, uri: &str) -> Option<String> {
    let raw = uri.strip_prefix("file://")?;
    let path = Path::new(raw);
    if !path.starts_with(wiki_dir) {
        return None;
    }
    Some(relative_slash_path(wiki_dir, path))
}

fn validate_plan(plan: &ConsistencyPlan) -> Result<(), DynError> {
    if plan.version != VERSION {
        return Err(format!("unsupported consistency plan version: {}", plan.version).into());
    }
    for action in &plan.actions {
        match action.kind {
            ConsistencyActionKind::DbFix => {
                if !matches!(
                    action.operation.as_str(),
                    "append_source_reference" | "replace_legacy_notion_link"
                ) || action.value.is_none()
                {
                    return Err(format!("invalid db_fix action: {}", action.path).into());
                }
            }
            ConsistencyActionKind::VaultCleanup => {
                if action.operation != "delete_unmanaged_empty_file" {
                    return Err(format!("invalid vault_cleanup action: {}", action.path).into());
                }
            }
            ConsistencyActionKind::PalaceReplay => {
                if action.operation != "replay_page_to_mempalace"
                    || !action.path.starts_with("wiki://page/")
                {
                    return Err(format!("invalid palace_replay action: {}", action.path).into());
                }
            }
            ConsistencyActionKind::NeedsHuman | ConsistencyActionKind::Deferred => {}
        }
    }
    Ok(())
}

fn validate_plan_against_audit(
    plan: &ConsistencyPlan,
    audit: &ConsistencyAuditReport,
) -> Result<(), DynError> {
    let cleanup_paths: BTreeSet<_> = audit
        .candidates
        .safe_cleanup_candidates
        .iter()
        .cloned()
        .collect();
    let palace_paths: BTreeSet<_> = audit.palace.missing_page_drawers.iter().cloned().collect();
    let source_refs: BTreeSet<_> = audit
        .candidates
        .source_summary_exact_matches
        .iter()
        .map(|candidate| {
            (
                format!("wiki://page/{}", candidate.summary_page_id),
                candidate.source_uri.clone(),
            )
        })
        .collect();
    let legacy_link_replacements: BTreeSet<_> = audit
        .vault
        .stale_notion_links
        .iter()
        .filter_map(|candidate| {
            if candidate.resolution.as_deref() != Some("notion_uuid_page") {
                return None;
            }
            Some((
                format!("wiki://page/{}", candidate.page_id),
                format!(
                    "{}\t{}",
                    candidate.raw_target,
                    candidate.replacement_target.clone()?
                ),
            ))
        })
        .collect();
    let known_pages: BTreeSet<_> = audit
        .db
        .page_ids
        .iter()
        .map(|id| format!("wiki://page/{id}"))
        .collect();

    for action in &plan.actions {
        match action.kind {
            ConsistencyActionKind::VaultCleanup => {
                if !cleanup_paths.contains(&action.path) {
                    return Err(format!(
                        "vault_cleanup path outside audit evidence: {}",
                        action.path
                    )
                    .into());
                }
            }
            ConsistencyActionKind::PalaceReplay => {
                if !palace_paths.contains(&action.path) {
                    return Err(format!(
                        "palace_replay path outside audit evidence: {}",
                        action.path
                    )
                    .into());
                }
            }
            ConsistencyActionKind::DbFix => {
                let value = action.value.clone().unwrap_or_default();
                match action.operation.as_str() {
                    "append_source_reference" => {
                        if !source_refs.contains(&(action.path.clone(), value)) {
                            return Err(format!(
                                "db_fix outside exact source-summary evidence: {}",
                                action.path
                            )
                            .into());
                        }
                    }
                    "replace_legacy_notion_link" => {
                        if !legacy_link_replacements.contains(&(action.path.clone(), value)) {
                            return Err(format!(
                                "db_fix outside legacy link evidence: {}",
                                action.path
                            )
                            .into());
                        }
                    }
                    _ => {
                        return Err(format!("invalid db_fix operation: {}", action.operation).into())
                    }
                }
            }
            ConsistencyActionKind::NeedsHuman | ConsistencyActionKind::Deferred => {
                if action.path.starts_with("wiki://page/") && !known_pages.contains(&action.path) {
                    return Err(format!(
                        "non-executable action path outside DB pages: {}",
                        action.path
                    )
                    .into());
                }
            }
        }
    }
    Ok(())
}

fn apply_db_fix(
    store: &mut InMemoryStore,
    action: &ConsistencyPlanAction,
) -> Result<bool, DynError> {
    let page_id = action
        .path
        .strip_prefix("wiki://page/")
        .ok_or_else(|| format!("db_fix path must be wiki://page/<id>: {}", action.path))?;
    let value = action.value.as_deref().ok_or("db_fix requires value")?;
    let Some(page) = store
        .pages
        .values_mut()
        .find(|page| page.id.0.to_string() == page_id)
    else {
        return Err(format!("page not found for db_fix: {page_id}").into());
    };
    if action.operation == "replace_legacy_notion_link" {
        let (raw_target, replacement_target) = value
            .split_once('\t')
            .ok_or("replace_legacy_notion_link value must be raw<TAB>replacement")?;
        if !page.markdown.contains(raw_target) {
            return Ok(false);
        }
        page.markdown = page.markdown.replace(raw_target, replacement_target);
        page.refresh_outbound_links();
        page.updated_at = OffsetDateTime::now_utc();
        return Ok(true);
    }
    let source_uri = value;
    if page.markdown.contains(source_uri) {
        return Ok(false);
    }
    page.markdown.push_str("\n\n## 来源\n\n");
    page.markdown.push_str(&format!("- `{source_uri}`\n"));
    page.refresh_outbound_links();
    page.updated_at = OffsetDateTime::now_utc();
    Ok(true)
}

fn apply_vault_cleanup(wiki_dir: &Path, action: &ConsistencyPlanAction) -> Result<bool, DynError> {
    let rel = action.path.trim_start_matches('/');
    if rel.contains("..") || !(rel.starts_with("pages/") || rel.starts_with("sources/")) {
        return Err(format!("unsafe vault cleanup path: {}", action.path).into());
    }
    let path = wiki_dir.join(rel);
    if !path.exists() {
        return Ok(false);
    }
    let meta = fs::metadata(&path)?;
    if !meta.is_file() || meta.len() != 0 {
        return Err(format!(
            "refusing to delete non-empty or non-file path: {}",
            action.path
        )
        .into());
    }
    fs::remove_file(path)?;
    Ok(true)
}

fn project_db_fixed_pages_to_vault(
    wiki_dir: &Path,
    store: &InMemoryStore,
    page_ids: &BTreeSet<String>,
) -> Result<usize, DynError> {
    let page_paths = vault_page_paths_by_id(wiki_dir)?;
    let mut written = 0usize;
    for page_id in page_ids {
        let page = store
            .pages
            .values()
            .find(|page| page.id.0.to_string() == page_id.as_str())
            .ok_or_else(|| format!("page not found for vault projection: {page_id}"))?;
        let Some(path) = page_paths.get(page_id).cloned() else {
            continue;
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let next = replace_markdown_body_preserving_frontmatter(&existing, &page.markdown, page);
        if existing != next {
            fs::write(path, next)?;
            written += 1;
        }
    }
    Ok(written)
}

fn vault_page_paths_by_id(wiki_dir: &Path) -> Result<BTreeMap<String, PathBuf>, DynError> {
    let mut out = BTreeMap::new();
    let root = wiki_dir.join("pages");
    if !root.exists() {
        return Ok(out);
    }
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Some(id) =
            frontmatter_value(&text, "page_id").or_else(|| frontmatter_value(&text, "id"))
        {
            out.insert(id, entry.path().to_path_buf());
        }
    }
    Ok(out)
}

fn replace_markdown_body_preserving_frontmatter(
    existing: &str,
    markdown: &str,
    page: &wiki_core::WikiPage,
) -> String {
    if let Some((frontmatter, _body)) = split_frontmatter(existing) {
        return format!("{frontmatter}\n\n{markdown}");
    }
    render_minimal_page_with_frontmatter(page)
}

fn split_frontmatter(markdown: &str) -> Option<(&str, &str)> {
    let rest = markdown.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let frontmatter_end = end + "---\n".len() + "\n---".len();
    let body_start = if markdown[frontmatter_end..].starts_with("\n\n") {
        frontmatter_end + 2
    } else if markdown[frontmatter_end..].starts_with('\n') {
        frontmatter_end + 1
    } else {
        frontmatter_end
    };
    Some((&markdown[..frontmatter_end], &markdown[body_start..]))
}

fn render_minimal_page_with_frontmatter(page: &wiki_core::WikiPage) -> String {
    format!(
        "---\nid: \"{}\"\ntitle: \"{}\"\nstatus: {}\nentry_type: {}\n---\n\n{}",
        page.id.0,
        page.title.replace('\\', "\\\\").replace('"', "\\\""),
        status_label(page.status),
        page.entry_type
            .as_ref()
            .map(entry_type_label)
            .unwrap_or("null"),
        page.markdown
    )
}

fn status_label(status: wiki_core::EntryStatus) -> &'static str {
    match status {
        wiki_core::EntryStatus::Draft => "draft",
        wiki_core::EntryStatus::InReview => "in_review",
        wiki_core::EntryStatus::Approved => "approved",
        wiki_core::EntryStatus::NeedsUpdate => "needs_update",
    }
}

fn action_kind_str(kind: ConsistencyActionKind) -> &'static str {
    match kind {
        ConsistencyActionKind::DbFix => "db_fix",
        ConsistencyActionKind::VaultCleanup => "vault_cleanup",
        ConsistencyActionKind::PalaceReplay => "palace_replay",
        ConsistencyActionKind::NeedsHuman => "needs_human",
        ConsistencyActionKind::Deferred => "deferred",
    }
}

fn find_unresolved_local_links(
    pages: &[DbPageEvidence],
    page_titles: &BTreeSet<String>,
) -> Vec<UnresolvedLocalLink> {
    let mut out = Vec::new();
    for page in pages {
        for target in extract_wikilinks(&page.markdown) {
            if target.contains("://") || target.contains('%') {
                continue;
            }
            if !page_titles.contains(&target) {
                out.push(UnresolvedLocalLink {
                    page_id: page.id.clone(),
                    page_title: page.title.clone(),
                    target,
                });
            }
        }
    }
    out
}

fn markdown_link_targets(markdown: &str) -> Vec<String> {
    let mut out = extract_wikilinks(markdown);
    let bytes = markdown.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b']' || i + 1 >= bytes.len() || bytes[i + 1] != b'(' {
            i += 1;
            continue;
        }
        let start = i + 2;
        let mut j = start;
        let mut depth = 0usize;
        let mut end = None;
        while j < bytes.len() {
            match bytes[j] {
                b'(' => depth += 1,
                b')' if depth == 0 => {
                    end = Some(j);
                    break;
                }
                b')' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        if let Some(end) = end {
            let target = markdown[start..end].trim();
            if !target.is_empty() {
                out.push(target.to_string());
            }
            i = end + 1;
        } else {
            break;
        }
    }
    out
}

fn local_target_exists(
    raw_target: &str,
    decoded_target: &str,
    known_paths: &BTreeSet<String>,
) -> bool {
    [raw_target, decoded_target]
        .iter()
        .map(|target| strip_fragment(target))
        .map(normalize_local_target)
        .any(|target| known_paths.contains(&target))
}

fn strip_fragment(target: &str) -> &str {
    target.split_once('#').map_or(target, |(path, _)| path)
}

fn normalize_local_target(target: &str) -> String {
    let mut out = target.trim_start_matches("./");
    while let Some(rest) = out.strip_prefix("../") {
        out = rest;
    }
    out.trim_start_matches('/').to_string()
}

fn notion_uuid_from_target(target: &str) -> Option<String> {
    let bytes = target.as_bytes();
    for window in bytes.windows(32) {
        if window.iter().all(u8::is_ascii_hexdigit) {
            return Some(String::from_utf8_lossy(window).to_ascii_lowercase());
        }
    }
    None
}

fn relative_target_from_page(entry_type: Option<&str>, target_path: &str) -> String {
    if let Some(rest) = target_path.strip_prefix("pages/") {
        return format!("../{rest}");
    }
    if target_path.starts_with("sources/") {
        return format!("../../{target_path}");
    }
    let _ = entry_type;
    target_path.to_string()
}

fn is_retired_notion_system_page(decoded_target: &str) -> bool {
    [
        "Wiki Schema（规则文件） 5616b84751134607a28a810ec9b26386.md",
        "系统工作流程图 49c4c53d95f0455e9c6669bdb79108cf.md",
    ]
    .contains(&decoded_target)
}

fn frontmatter_value(markdown: &str, key: &str) -> Option<String> {
    let rest = markdown.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let Some((raw_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        if raw_key.trim() != key {
            continue;
        }
        let value = raw_value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

fn percent_decode(input: &str) -> String {
    if !input.contains('%') {
        return input.to_string();
    }
    let mut out = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn looks_like_notion_export_filename(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.ends_with(".md")
        && target
            .split(|c: char| !c.is_ascii_hexdigit())
            .any(|part| part.len() >= 32)
}

fn normalized_title(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn filename_timestamp(generated_at: &str) -> String {
    generated_at.replace([':', '-', '.'], "")
}

fn relative_slash_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
