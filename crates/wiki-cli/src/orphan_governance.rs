use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use walkdir::WalkDir;

const CLEANUP_WHITELIST: &[&str] = &[
    ".DS_Store",
    "concepts/7433d289.md",
    "concepts/",
    "_archive/legacy-root/AGENTS md 5da673ca2377484498ec12f5679bfbf3.md",
    "_archive/legacy-root/concepts/04ff4434.md",
    "_archive/legacy-root/concepts/",
    "_archive/legacy-root/analyses/",
    ".wiki/orphan-audit-report.md",
    "reports/vault-audit.json",
    "reports/vault-audit.md",
];

#[derive(Debug, Clone, Serialize)]
pub struct OrphanGovernancePlanFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrphanGovernanceApplyReport {
    pub mode: &'static str,
    pub plan_path: String,
    pub actions_seen: usize,
    pub executable_actions: usize,
    pub page_status_insertions_planned: usize,
    pub source_compiled_to_wiki_insertions_planned: usize,
    pub cleanup_deletions_planned: usize,
    pub page_status_insertions_applied: usize,
    pub source_compiled_to_wiki_insertions_applied: usize,
    pub cleanup_deletions_applied: usize,
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanGovernancePlan {
    #[serde(default, deserialize_with = "null_to_zero_u32")]
    pub version: u32,
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub generated_at: String,
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub audit_report_path: String,
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub vault_path: String,
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub audit_generated_at: String,
    pub actions: Vec<GovernanceAction>,
    #[serde(default)]
    pub markdown_report_model: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceAction {
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub action_type: String,
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub path: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default, deserialize_with = "null_to_zero_f64")]
    pub confidence: f64,
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub reason: String,
    #[serde(default, deserialize_with = "null_to_empty_string")]
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
struct PlannerEvidence {
    generated_at: String,
    audit_report_path: String,
    vault_path: String,
    audit_generated_at: String,
    counts: GovernanceCounts,
    page_missing_status_paths: Vec<String>,
    source_missing_compiled_to_wiki_paths: Vec<String>,
    unsupported_frontmatter_paths: Vec<String>,
    orphan_candidate_paths: Vec<String>,
    cleanup_candidates: Vec<String>,
    source_title_candidates: Vec<TitleCandidate>,
    summary_title_candidates: Vec<TitleCandidate>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GovernanceCounts {
    pub orphan_candidates: usize,
    pub unsupported_frontmatter: usize,
    pub pages_missing_status: usize,
    pub sources_missing_compiled_to_wiki: usize,
}

#[derive(Debug, Clone, Serialize)]
struct TitleCandidate {
    path: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuditInput {
    vault_path: Option<String>,
    generated_at: Option<String>,
    orphan_candidates: Option<AuditOrphanCandidates>,
    readiness: Option<AuditReadiness>,
    pages: Option<AuditPages>,
    sources: Option<AuditSources>,
    path_lists: Option<AuditPathLists>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditOrphanCandidates {
    total_files: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditReadiness {
    unsupported_frontmatter: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditPages {
    missing_status: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditSources {
    compiled_to_wiki: Option<AuditBoolStats>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditBoolStats {
    missing: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditPathLists {
    pages_missing_status: Option<Vec<String>>,
    sources_missing_compiled_to_wiki: Option<Vec<String>>,
    unsupported_frontmatter: Option<Vec<String>>,
    orphan_candidates: Option<Vec<String>>,
}

fn null_to_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn null_to_zero_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or_default())
}

fn null_to_zero_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u32>::deserialize(deserializer)?.unwrap_or_default())
}

#[allow(dead_code)]
pub fn run_plan_with_llm<F>(
    audit_report: impl AsRef<Path>,
    report_dir: Option<PathBuf>,
    wiki_dir: Option<&Path>,
    mut complete_chat: F,
) -> Result<
    (OrphanGovernancePlan, OrphanGovernancePlanFiles),
    Box<dyn std::error::Error + Send + Sync>,
>
where
    F: FnMut(&str, &str, u32) -> Result<String, Box<dyn std::error::Error + Send + Sync>>,
{
    let audit_report = audit_report.as_ref();
    let evidence = build_evidence_from_audit_path(audit_report, wiki_dir)?;
    let plan_reply = complete_chat(
        planner_system_prompt(),
        &planner_user_prompt(&evidence)?,
        8192,
    )?;
    let mut plan = validate_plan_json(&plan_reply, &evidence)?;
    plan.generated_at = evidence.generated_at.clone();
    let markdown = render_validated_plan_markdown(&plan);
    validate_markdown_report(&markdown, &plan)?;
    let report_dir = resolve_report_dir(audit_report, report_dir, wiki_dir)?;
    let files = write_plan_files(&plan, &markdown, &report_dir)?;
    Ok((plan, files))
}

#[allow(dead_code)]
pub fn build_plan_from_llm_outputs(
    audit_report: impl AsRef<Path>,
    report_dir: Option<PathBuf>,
    wiki_dir: Option<&Path>,
    plan_json: &str,
    _markdown: &str,
) -> Result<
    (OrphanGovernancePlan, OrphanGovernancePlanFiles),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let audit_report = audit_report.as_ref();
    let evidence = build_evidence_from_audit_path(audit_report, wiki_dir)?;
    let mut plan = validate_plan_json(plan_json, &evidence)?;
    plan.generated_at = evidence.generated_at.clone();
    let markdown = render_validated_plan_markdown(&plan);
    validate_markdown_report(&markdown, &plan)?;
    let report_dir = resolve_report_dir(audit_report, report_dir, wiki_dir)?;
    let files = write_plan_files(&plan, &markdown, &report_dir)?;
    Ok((plan, files))
}

pub fn run_apply(
    plan_path: impl AsRef<Path>,
    wiki_dir: &Path,
    apply: bool,
) -> Result<OrphanGovernanceApplyReport, Box<dyn std::error::Error + Send + Sync>> {
    let plan_path = plan_path.as_ref();
    let body = fs::read_to_string(plan_path)?;
    let parsed_plan: OrphanGovernancePlan = serde_json::from_str(&body)?;
    let evidence =
        build_evidence_from_audit_path(Path::new(&parsed_plan.audit_report_path), Some(wiki_dir))?;
    let mut plan = validate_plan_json(&serde_json::to_string(&parsed_plan)?, &evidence)?;
    plan.generated_at = parsed_plan.generated_at;
    validate_apply_plan(&plan)?;

    let mut report = OrphanGovernanceApplyReport {
        mode: if apply { "apply" } else { "dry_run" },
        plan_path: plan_path.display().to_string(),
        actions_seen: plan.actions.len(),
        executable_actions: 0,
        page_status_insertions_planned: 0,
        source_compiled_to_wiki_insertions_planned: 0,
        cleanup_deletions_planned: 0,
        page_status_insertions_applied: 0,
        source_compiled_to_wiki_insertions_applied: 0,
        cleanup_deletions_applied: 0,
        skipped: Vec::new(),
    };

    for action in &plan.actions {
        match action.action_type.as_str() {
            "insert_page_status" => {
                report.executable_actions += 1;
                report.page_status_insertions_planned += 1;
                if apply {
                    let value = required_string_value(action, "insert_page_status")?;
                    let path = safe_existing_vault_path(wiki_dir, &action.path)?;
                    if insert_frontmatter_key_if_missing(&path, "status", value)? {
                        report.page_status_insertions_applied += 1;
                    } else {
                        report
                            .skipped
                            .push(format!("status already present: {}", action.path));
                    }
                }
            }
            "insert_source_compiled_to_wiki" => {
                report.executable_actions += 1;
                report.source_compiled_to_wiki_insertions_planned += 1;
                if apply {
                    let value = required_bool_value(action, "insert_source_compiled_to_wiki")?;
                    let path = safe_existing_vault_path(wiki_dir, &action.path)?;
                    if insert_frontmatter_key_if_missing(&path, "compiled_to_wiki", value)? {
                        report.source_compiled_to_wiki_insertions_applied += 1;
                    } else {
                        report
                            .skipped
                            .push(format!("compiled_to_wiki already present: {}", action.path));
                    }
                }
            }
            "delete_cleanup_path" => {
                report.executable_actions += 1;
                report.cleanup_deletions_planned += 1;
                if apply {
                    let path = wiki_dir.join(&action.path);
                    if path.exists() {
                        let path = safe_existing_vault_path(wiki_dir, &action.path)?;
                        if path.is_dir() {
                            fs::remove_dir_all(&path)?;
                        } else {
                            fs::remove_file(&path)?;
                        }
                        report.cleanup_deletions_applied += 1;
                    } else {
                        report
                            .skipped
                            .push(format!("cleanup path missing: {}", action.path));
                    }
                }
            }
            "needs_human" | "recommend_batch_ingest" => {}
            other => {
                return Err(invalid_data(format!(
                    "unknown governance action_type in apply plan: {other}"
                ))
                .into());
            }
        }
    }

    Ok(report)
}

fn build_evidence_from_audit_path(
    audit_report: &Path,
    wiki_dir: Option<&Path>,
) -> Result<PlannerEvidence, Box<dyn std::error::Error + Send + Sync>> {
    validate_timestamped_audit_path(audit_report)?;
    let body = fs::read_to_string(audit_report)?;
    let audit: AuditInput = serde_json::from_str(&body)?;
    validate_audit_report_matches_wiki_dir(audit_report, audit.vault_path.as_deref(), wiki_dir)?;
    let audit_generated_at = require_field(audit.generated_at.clone(), "generated_at")?;
    let counts = build_counts(&audit)?;
    let vault_path = wiki_dir
        .map(|p| p.display().to_string())
        .or_else(|| audit.vault_path.clone())
        .ok_or_else(|| invalid_data("audit report missing required field: vault_path"))?;
    let wiki_dir_path = Path::new(&vault_path);

    let path_lists = audit.path_lists.unwrap_or_default();
    let mut page_missing_status_paths =
        normalize_path_list(path_lists.pages_missing_status.unwrap_or_default());
    page_missing_status_paths.retain(|path| path.starts_with("pages/"));

    let mut source_missing_compiled_to_wiki_paths = normalize_path_list(
        path_lists
            .sources_missing_compiled_to_wiki
            .unwrap_or_default(),
    );
    source_missing_compiled_to_wiki_paths.retain(|path| path.starts_with("sources/"));

    let unsupported_frontmatter_paths =
        normalize_path_list(path_lists.unsupported_frontmatter.unwrap_or_default());

    let orphan_candidate_paths =
        normalize_path_list(path_lists.orphan_candidates.unwrap_or_default());

    let cleanup_candidates = cleanup_candidates(wiki_dir_path);
    let source_missing_paths = source_missing_compiled_to_wiki_paths
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let source_title_candidates = title_candidates_for_paths(wiki_dir_path, &source_missing_paths)?;
    let all_summary_candidates = title_candidates(wiki_dir_path, "pages/summary")?;
    let summary_title_candidates =
        matching_summary_candidates(&source_title_candidates, &all_summary_candidates);

    Ok(PlannerEvidence {
        generated_at: now_rfc3339(),
        audit_report_path: audit_report.display().to_string(),
        vault_path,
        audit_generated_at,
        counts,
        page_missing_status_paths: page_missing_status_paths.into_iter().collect(),
        source_missing_compiled_to_wiki_paths: source_missing_compiled_to_wiki_paths
            .into_iter()
            .collect(),
        unsupported_frontmatter_paths: unsupported_frontmatter_paths.into_iter().collect(),
        orphan_candidate_paths: orphan_candidate_paths.into_iter().collect(),
        cleanup_candidates,
        source_title_candidates,
        summary_title_candidates,
    })
}

fn build_counts(audit: &AuditInput) -> io::Result<GovernanceCounts> {
    let orphan_candidates = require_field(audit.orphan_candidates.as_ref(), "orphan_candidates")?;
    let readiness = require_field(audit.readiness.as_ref(), "readiness")?;
    let pages = require_field(audit.pages.as_ref(), "pages")?;
    let sources = require_field(audit.sources.as_ref(), "sources")?;
    let compiled_to_wiki = require_field(
        sources.compiled_to_wiki.as_ref(),
        "sources.compiled_to_wiki",
    )?;
    Ok(GovernanceCounts {
        orphan_candidates: require_field(
            orphan_candidates.total_files,
            "orphan_candidates.total_files",
        )?,
        unsupported_frontmatter: require_field(
            readiness.unsupported_frontmatter,
            "readiness.unsupported_frontmatter",
        )?,
        pages_missing_status: require_field(pages.missing_status, "pages.missing_status")?,
        sources_missing_compiled_to_wiki: require_field(
            compiled_to_wiki.missing,
            "sources.compiled_to_wiki.missing",
        )?,
    })
}

fn validate_plan_json(
    raw: &str,
    evidence: &PlannerEvidence,
) -> Result<OrphanGovernancePlan, Box<dyn std::error::Error + Send + Sync>> {
    let json_slice = parse_json_object_slice(raw);
    let mut plan: OrphanGovernancePlan = serde_json::from_str(json_slice)?;
    if plan.version != 1 {
        return Err(invalid_data(format!(
            "unsupported governance plan version: {}",
            plan.version
        ))
        .into());
    }
    require_equal(
        &plan.audit_report_path,
        &evidence.audit_report_path,
        "audit_report_path",
    )?;
    require_equal(&plan.vault_path, &evidence.vault_path, "vault_path")?;
    require_equal(
        &plan.audit_generated_at,
        &evidence.audit_generated_at,
        "audit_generated_at",
    )?;

    let all_evidence_paths = allowed_evidence_paths(evidence);
    let page_missing: BTreeSet<_> = evidence.page_missing_status_paths.iter().cloned().collect();
    let source_missing: BTreeSet<_> = evidence
        .source_missing_compiled_to_wiki_paths
        .iter()
        .cloned()
        .collect();
    let cleanup: BTreeSet<_> = evidence.cleanup_candidates.iter().cloned().collect();

    let mut validated_actions = Vec::with_capacity(plan.actions.len());
    for mut action in plan.actions {
        if action.reason.trim().is_empty() {
            action.reason = "LLM omitted reason; path and value validated by program.".to_string();
        }
        if action.source.trim().is_empty() {
            action.source = "llm".to_string();
        }
        if action.path.trim().is_empty() {
            match action.action_type.as_str() {
                "" | "needs_human" | "recommend_batch_ingest" => continue,
                _ => {
                    return Err(invalid_data(format!(
                        "governance action path is missing for {}",
                        action.action_type
                    ))
                    .into());
                }
            }
        }
        let normalized = normalize_rel_path(&action.path).ok_or_else(|| {
            invalid_data(format!(
                "governance action path is not vault-relative: {}",
                action.path
            ))
        })?;
        action.path = normalized;
        if !all_evidence_paths.contains(&action.path) {
            match action.action_type.as_str() {
                "needs_human" | "recommend_batch_ingest" => continue,
                _ => {
                    return Err(invalid_data(format!(
                        "governance action references path outside evidence: {}",
                        action.path
                    ))
                    .into());
                }
            }
        }
        validate_common_action_fields(&action)?;
        match action.action_type.as_str() {
            "insert_page_status" => {
                if !page_missing.contains(&action.path) {
                    return Err(invalid_data(format!(
                        "insert_page_status path not in missing-status evidence: {}",
                        action.path
                    ))
                    .into());
                }
                let value = required_string_value(&action, "insert_page_status")?;
                validate_status_value(value)?;
            }
            "insert_source_compiled_to_wiki" => {
                if !source_missing.contains(&action.path) {
                    return Err(invalid_data(format!(
                        "insert_source_compiled_to_wiki path not in missing-compiled evidence: {}",
                        action.path
                    ))
                    .into());
                }
                required_bool_value(&action, "insert_source_compiled_to_wiki")?;
            }
            "delete_cleanup_path" => {
                if !cleanup.contains(&action.path) {
                    return Err(invalid_data(format!(
                        "delete_cleanup_path path not in cleanup whitelist candidates: {}",
                        action.path
                    ))
                    .into());
                }
                action.value = None;
            }
            "needs_human" | "recommend_batch_ingest" => {}
            other => {
                return Err(
                    invalid_data(format!("unknown governance action_type: {other}")).into(),
                );
            }
        }
        validated_actions.push(action);
    }
    plan.actions = validated_actions;
    Ok(plan)
}

fn validate_apply_plan(plan: &OrphanGovernancePlan) -> io::Result<()> {
    if plan.version != 1 {
        return Err(invalid_data(format!(
            "unsupported governance plan version: {}",
            plan.version
        )));
    }
    for action in &plan.actions {
        validate_common_action_fields(action)?;
        if normalize_rel_path(&action.path).as_deref() != Some(action.path.as_str()) {
            return Err(invalid_data(format!(
                "governance action path is not vault-relative: {}",
                action.path
            )));
        }
        match action.action_type.as_str() {
            "insert_page_status" => {
                if !action.path.starts_with("pages/") {
                    return Err(invalid_data(format!(
                        "insert_page_status path must be under pages/: {}",
                        action.path
                    )));
                }
                validate_status_value(required_string_value(action, "insert_page_status")?)?;
            }
            "insert_source_compiled_to_wiki" => {
                if !action.path.starts_with("sources/") {
                    return Err(invalid_data(format!(
                        "insert_source_compiled_to_wiki path must be under sources/: {}",
                        action.path
                    )));
                }
                required_bool_value(action, "insert_source_compiled_to_wiki")?;
            }
            "delete_cleanup_path" => {
                if !cleanup_whitelist_contains(&action.path) {
                    return Err(invalid_data(format!(
                        "delete_cleanup_path path not in cleanup whitelist: {}",
                        action.path
                    )));
                }
                if action
                    .value
                    .as_ref()
                    .is_some_and(|value| !is_empty_delete_value(value))
                {
                    return Err(invalid_data("delete_cleanup_path must not set value"));
                }
            }
            "needs_human" | "recommend_batch_ingest" => {}
            other => {
                return Err(invalid_data(format!(
                    "unknown governance action_type: {other}"
                )))
            }
        }
    }
    Ok(())
}

fn validate_common_action_fields(action: &GovernanceAction) -> io::Result<()> {
    if !(0.0..=1.0).contains(&action.confidence) {
        return Err(invalid_data(format!(
            "governance action confidence out of range for {}",
            action.path
        )));
    }
    if action.reason.trim().is_empty() {
        return Err(invalid_data(format!(
            "governance action reason missing for {}",
            action.path
        )));
    }
    if !matches!(action.source.as_str(), "rule" | "llm") {
        return Err(invalid_data(format!(
            "governance action source must be rule or llm for {}",
            action.path
        )));
    }
    Ok(())
}

fn validate_markdown_report(markdown: &str, plan: &OrphanGovernancePlan) -> io::Result<()> {
    let trimmed = markdown.trim();
    if trimmed.is_empty() {
        return Err(invalid_data("governance markdown report is empty"));
    }
    if trimmed.contains("```") || contains_shell_command(trimmed) {
        return Err(invalid_data(
            "governance markdown report must not include executable commands",
        ));
    }
    if !trimmed
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    {
        return Err(invalid_data("governance markdown report must be Chinese"));
    }
    validate_markdown_paths(trimmed, plan)?;
    Ok(())
}

fn render_validated_plan_markdown(plan: &OrphanGovernancePlan) -> String {
    let page_status = plan
        .actions
        .iter()
        .filter(|action| action.action_type == "insert_page_status")
        .collect::<Vec<_>>();
    let source_compiled = plan
        .actions
        .iter()
        .filter(|action| action.action_type == "insert_source_compiled_to_wiki")
        .collect::<Vec<_>>();
    let cleanup = plan
        .actions
        .iter()
        .filter(|action| action.action_type == "delete_cleanup_path")
        .collect::<Vec<_>>();
    let advisory = plan
        .actions
        .iter()
        .filter(|action| {
            matches!(
                action.action_type.as_str(),
                "needs_human" | "recommend_batch_ingest"
            )
        })
        .collect::<Vec<_>>();

    let mut out = String::new();
    out.push_str("# 孤儿治理计划\n\n");
    out.push_str(&format!("- 生成时间：{}\n", plan.generated_at));
    out.push_str(&format!("- 审计时间：{}\n", plan.audit_generated_at));
    out.push_str(&format!("- 总动作：{}\n", plan.actions.len()));
    out.push_str(&format!(
        "- 可自动执行：{}\n\n",
        page_status.len() + source_compiled.len() + cleanup.len()
    ));

    out.push_str(&format!(
        "## 补 page status（{} 项）\n\n",
        page_status.len()
    ));
    render_action_list(&mut out, &page_status);

    out.push_str(&format!(
        "\n## 补 source compiled_to_wiki（{} 项）\n\n",
        source_compiled.len()
    ));
    render_action_list(&mut out, &source_compiled);

    out.push_str(&format!("\n## 删除清理白名单（{} 项）\n\n", cleanup.len()));
    render_action_list(&mut out, &cleanup);

    if !advisory.is_empty() {
        out.push_str(&format!("\n## 只报告不执行（{} 项）\n\n", advisory.len()));
        render_action_list(&mut out, &advisory);
    }

    out.push_str("\n## 边界\n\n");
    out.push_str("- 本计划不执行 batch-ingest。\n");
    out.push_str("- 本计划不改正文。\n");
    out.push_str("- apply 只执行以上白名单动作。\n");
    out
}

fn render_action_list(out: &mut String, actions: &[&GovernanceAction]) {
    if actions.is_empty() {
        out.push_str("- 无\n");
        return;
    }
    for action in actions {
        match action.action_type.as_str() {
            "insert_page_status" | "insert_source_compiled_to_wiki" => {
                let value = action
                    .value
                    .as_ref()
                    .map(Value::to_string)
                    .unwrap_or_else(|| "null".to_string());
                out.push_str(&format!("- `{}` -> `{}`\n", action.path, value));
            }
            _ => out.push_str(&format!("- `{}`\n", action.path)),
        }
    }
}

fn validate_markdown_paths(markdown: &str, plan: &OrphanGovernancePlan) -> io::Result<()> {
    let allowed_paths: BTreeSet<&str> = plan
        .actions
        .iter()
        .map(|action| action.path.as_str())
        .collect();
    let mut residual = markdown.to_string();
    for path in &allowed_paths {
        residual = residual.replace(path, "");
    }
    for token in residual.split_whitespace() {
        let token = token.trim_matches(|ch: char| {
            matches!(
                ch,
                '`' | '\''
                    | '"'
                    | '，'
                    | '。'
                    | '、'
                    | '：'
                    | ':'
                    | ','
                    | '.'
                    | ')'
                    | '('
                    | '['
                    | ']'
            )
        });
        if !looks_like_vault_path(token) {
            continue;
        }
        let normalized = normalize_rel_path(token).ok_or_else(|| {
            invalid_data(format!(
                "governance markdown report includes invalid path: {token}"
            ))
        })?;
        if !allowed_paths.contains(normalized.as_str()) {
            return Err(invalid_data(format!(
                "governance markdown report includes path not present in validated plan: {normalized}"
            )));
        }
    }
    Ok(())
}

fn contains_shell_command(markdown: &str) -> bool {
    let command_prefixes = [
        "$ ",
        "cargo ",
        "wiki-cli ",
        "rm ",
        "rm\t",
        "sh ",
        "bash ",
        "zsh ",
        "python ",
        "python3 ",
        "node ",
        "npm ",
        "npx ",
        "pnpm ",
        "uv ",
        "git ",
        "find ",
        "xargs ",
        "sed ",
        "awk ",
        "perl ",
    ];
    markdown.lines().any(|line| {
        let line = line.trim_start();
        command_prefixes
            .iter()
            .any(|prefix| line.starts_with(prefix))
    })
}

fn looks_like_vault_path(token: &str) -> bool {
    token.starts_with("pages/")
        || token.starts_with("sources/")
        || token.starts_with("reports/")
        || token.starts_with("_archive/")
        || token.starts_with(".wiki/")
        || token.starts_with("concepts/")
        || token.starts_with("/Users/")
        || token.starts_with("/private/")
        || token.starts_with("/var/")
}

fn allowed_evidence_paths(evidence: &PlannerEvidence) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    paths.extend(evidence.page_missing_status_paths.iter().cloned());
    paths.extend(
        evidence
            .source_missing_compiled_to_wiki_paths
            .iter()
            .cloned(),
    );
    paths.extend(evidence.unsupported_frontmatter_paths.iter().cloned());
    paths.extend(evidence.orphan_candidate_paths.iter().cloned());
    paths.extend(evidence.cleanup_candidates.iter().cloned());
    paths.extend(
        evidence
            .source_title_candidates
            .iter()
            .map(|candidate| candidate.path.clone()),
    );
    paths.extend(
        evidence
            .summary_title_candidates
            .iter()
            .map(|candidate| candidate.path.clone()),
    );
    paths
}

pub fn resolve_report_dir(
    audit_report: &Path,
    report_dir: Option<PathBuf>,
    wiki_dir: Option<&Path>,
) -> io::Result<PathBuf> {
    if let Some(wiki_dir) = wiki_dir {
        let dir = match report_dir {
            Some(dir) if dir.is_absolute() => dir,
            Some(dir) => wiki_dir.join(dir),
            None => wiki_dir.join("reports"),
        };
        validate_under_wiki_reports(wiki_dir, &dir)?;
        return Ok(dir);
    }

    if let Some(dir) = report_dir {
        return normalize_path(dir);
    }
    let parent = audit_report.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "audit report has no parent directory: {}",
                audit_report.display()
            ),
        )
    })?;
    normalize_path(parent)
}

fn write_plan_files(
    plan: &OrphanGovernancePlan,
    markdown: &str,
    report_dir: &Path,
) -> Result<OrphanGovernancePlanFiles, Box<dyn std::error::Error + Send + Sync>> {
    fs::create_dir_all(report_dir)?;
    let timestamp = filename_timestamp(&plan.generated_at);
    let json_path = report_dir.join(format!("orphan-governance-plan-{timestamp}.json"));
    let markdown_path = report_dir.join(format!("orphan-governance-plan-{timestamp}.md"));
    fs::write(&json_path, serde_json::to_string_pretty(plan)?)?;
    fs::write(&markdown_path, markdown.trim_end())?;
    Ok(OrphanGovernancePlanFiles {
        json_path,
        markdown_path,
    })
}

#[allow(dead_code)]
fn planner_system_prompt() -> &'static str {
    r#"You are a strict orphan-governance planner for a Markdown vault.
Return ONLY one JSON object, no markdown fences.
Schema:
{
  "version": 1,
  "generated_at": "RFC3339 timestamp from evidence",
  "audit_report_path": "exact audit_report_path from evidence",
  "vault_path": "exact vault_path from evidence",
  "audit_generated_at": "exact audit_generated_at from evidence",
  "actions": [
    {
      "action_type": "insert_page_status|insert_source_compiled_to_wiki|delete_cleanup_path|needs_human|recommend_batch_ingest",
      "path": "one exact vault-relative path from evidence",
      "value": "draft, in_review, approved, needs_update, boolean, or omitted",
      "confidence": 0.0,
      "reason": "short reason",
      "source": "rule|llm"
    }
  ],
  "markdown_report_model": {}
}
Rules:
- Do not invent paths, counts, commands, files, or directories.
- Use only exact file paths present in evidence. Never use directory paths such as sources/x or pages/concept.
- Executable actions are only insert_page_status, insert_source_compiled_to_wiki, delete_cleanup_path.
- recommend_batch_ingest is a note only. Never tell code to run batch-ingest.
- delete_cleanup_path only for cleanup_candidates.
- Prefer status value "draft" when a page status is missing.
- Prefer compiled_to_wiki false when a source flag is missing and human compile decision is unknown."#
}

#[allow(dead_code)]
fn planner_user_prompt(evidence: &PlannerEvidence) -> serde_json::Result<String> {
    Ok(format!(
        "Build a governance plan from this evidence JSON:\n{}",
        serde_json::to_string_pretty(evidence)?
    ))
}

#[allow(dead_code)]
fn markdown_system_prompt() -> &'static str {
    r#"You write a concise Chinese Markdown report for a validated orphan-governance plan.
Return Markdown only. Do not add paths, counts, or commands not present in the JSON.
Mention that batch-ingest is not executed by this plan.
Do not include shell commands, cargo commands, wiki-cli commands, or absolute paths."#
}

#[allow(dead_code)]
fn markdown_user_prompt(plan_json: &str) -> String {
    format!("Render Chinese Markdown from this validated plan JSON:\n{plan_json}")
}

fn validate_timestamped_audit_path(path: &Path) -> io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            invalid_input(format!(
                "audit report path has no file name: {}",
                path.display()
            ))
        })?;
    if file_name == "vault-audit.json" {
        return Err(invalid_input("orphan-governance plan requires timestamped vault-audit-<timestamp>.json; got vault-audit.json"));
    }
    if file_name.starts_with("vault-audit-")
        && file_name.ends_with(".json")
        && file_name.len() > "vault-audit-.json".len()
    {
        return Ok(());
    }
    Err(invalid_input(format!(
        "orphan-governance plan requires timestamped vault-audit-<timestamp>.json; got {file_name}"
    )))
}

fn validate_audit_report_matches_wiki_dir(
    audit_report: &Path,
    audit_vault_path: Option<&str>,
    wiki_dir: Option<&Path>,
) -> io::Result<()> {
    let Some(wiki_dir) = wiki_dir else {
        return Ok(());
    };

    let canonical_wiki_dir = fs::canonicalize(wiki_dir)?;
    let reports_dir = canonical_wiki_dir.join("reports");
    let canonical_reports_dir = fs::canonicalize(&reports_dir)?;
    let canonical_audit_report = fs::canonicalize(audit_report)?;
    if !canonical_audit_report.starts_with(&canonical_reports_dir) {
        return Err(invalid_data(format!(
            "audit report must be under current wiki reports directory: {}",
            audit_report.display()
        )));
    }

    let audit_vault_path = audit_vault_path
        .ok_or_else(|| invalid_data("audit report missing required field: vault_path"))?;
    let canonical_audit_vault = fs::canonicalize(audit_vault_path)?;
    if canonical_audit_vault != canonical_wiki_dir {
        return Err(invalid_data(format!(
            "audit report vault_path does not match current wiki_dir: {}",
            audit_vault_path
        )));
    }
    Ok(())
}

fn normalize_path_list(paths: Vec<String>) -> BTreeSet<String> {
    paths
        .into_iter()
        .filter_map(|path| normalize_rel_path(&path))
        .collect()
}

fn cleanup_candidates(wiki_dir: &Path) -> Vec<String> {
    CLEANUP_WHITELIST
        .iter()
        .map(|path| path.trim_end_matches('/'))
        .filter(|path| wiki_dir.join(path).exists())
        .map(ToString::to_string)
        .collect()
}

fn cleanup_whitelist_contains(path: &str) -> bool {
    CLEANUP_WHITELIST
        .iter()
        .map(|item| item.trim_end_matches('/'))
        .any(|item| item == path)
}

fn title_candidates_for_paths(
    wiki_dir: &Path,
    rel_paths: &[String],
) -> io::Result<Vec<TitleCandidate>> {
    let mut candidates = Vec::new();
    for rel_path in rel_paths {
        let Some(rel_path) = normalize_rel_path(rel_path) else {
            continue;
        };
        let path = wiki_dir.join(&rel_path);
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let title = fs::read_to_string(&path)
            .ok()
            .and_then(|content| frontmatter_value(&content, "title").map(ToString::to_string));
        candidates.push(TitleCandidate {
            path: rel_path,
            title,
        });
    }
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(candidates)
}

fn title_candidates(wiki_dir: &Path, rel_dir: &str) -> io::Result<Vec<TitleCandidate>> {
    let root = wiki_dir.join(rel_dir);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut candidates = Vec::new();
    for entry in WalkDir::new(&root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("md")
        {
            continue;
        }
        let rel_path = relative_slash_path(wiki_dir, entry.path());
        let title = fs::read_to_string(entry.path())
            .ok()
            .and_then(|content| frontmatter_value(&content, "title").map(ToString::to_string));
        candidates.push(TitleCandidate {
            path: rel_path,
            title,
        });
    }
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(candidates)
}

fn matching_summary_candidates(
    sources: &[TitleCandidate],
    summaries: &[TitleCandidate],
) -> Vec<TitleCandidate> {
    let mut out = BTreeMap::new();
    for source in sources {
        let source_key = candidate_match_key(source);
        if source_key.len() < 4 {
            continue;
        }
        for summary in summaries {
            let summary_key = candidate_match_key(summary);
            if summary_key.len() < 4 {
                continue;
            }
            if source_key.contains(&summary_key) || summary_key.contains(&source_key) {
                out.entry(summary.path.clone())
                    .or_insert_with(|| summary.clone());
            }
            if out.len() >= sources.len().saturating_mul(3).max(10) {
                break;
            }
        }
    }
    out.into_values().collect()
}

fn candidate_match_key(candidate: &TitleCandidate) -> String {
    let raw = candidate.title.as_deref().unwrap_or(&candidate.path);
    raw.chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .flat_map(char::to_lowercase)
        .collect()
}

fn insert_frontmatter_key_if_missing(path: &Path, key: &str, value: &str) -> io::Result<bool> {
    let content = fs::read_to_string(path)?;
    if frontmatter_value(&content, key).is_some() {
        return Ok(false);
    }
    let rendered = if let Some((frontmatter, body)) = split_frontmatter(&content) {
        format!(
            "---\n{key}: {value}\n{}\n---\n{}",
            frontmatter.trim_end(),
            body
        )
    } else {
        format!("---\n{key}: {value}\n---\n\n{content}")
    };
    fs::write(path, rendered)?;
    Ok(true)
}

fn safe_existing_vault_path(wiki_dir: &Path, rel_path: &str) -> io::Result<PathBuf> {
    let path = wiki_dir.join(rel_path);
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid_data(format!(
            "governance apply refuses symlink path: {rel_path}"
        )));
    }
    let canonical_wiki_dir = fs::canonicalize(wiki_dir)?;
    let canonical_path = fs::canonicalize(&path)?;
    if !canonical_path.starts_with(&canonical_wiki_dir) {
        return Err(invalid_data(format!(
            "governance apply path escapes wiki_dir: {rel_path}"
        )));
    }
    Ok(canonical_path)
}

fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let idx = rest.find("\n---")?;
    let frontmatter = &rest[..idx];
    let after_marker = &rest[idx + "\n---".len()..];
    let body = after_marker
        .strip_prefix("\r\n")
        .or_else(|| after_marker.strip_prefix('\n'))
        .unwrap_or(after_marker);
    Some((frontmatter, body))
}

fn frontmatter_value<'a>(content: &'a str, key: &str) -> Option<&'a str> {
    let (frontmatter, _) = split_frontmatter(content)?;
    for line in frontmatter.lines() {
        let line = line.trim();
        let Some((raw_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        if raw_key.trim() == key {
            return Some(raw_value.trim().trim_matches('"').trim_matches('\''));
        }
    }
    None
}

fn required_string_value<'a>(
    action: &'a GovernanceAction,
    action_type: &str,
) -> io::Result<&'a str> {
    action
        .value
        .as_ref()
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_data(format!("{action_type} requires string value")))
}

fn required_bool_value(action: &GovernanceAction, action_type: &str) -> io::Result<&'static str> {
    match action.value.as_ref().and_then(Value::as_bool) {
        Some(true) => Ok("true"),
        Some(false) => Ok("false"),
        None => Err(invalid_data(format!(
            "{action_type} requires boolean value"
        ))),
    }
}

fn is_empty_delete_value(value: &Value) -> bool {
    value.is_null()
        || value.as_str().is_some_and(|s| {
            let s = s.trim().to_ascii_lowercase();
            s.is_empty() || s == "null" || s == "omitted" || s == "none"
        })
}

fn validate_status_value(value: &str) -> io::Result<()> {
    match value {
        "draft" | "in_review" | "approved" | "needs_update" => Ok(()),
        other => Err(invalid_data(format!("invalid page status value: {other}"))),
    }
}

fn require_equal(actual: &str, expected: &str, field: &str) -> io::Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(invalid_data(format!(
            "governance plan {field} mismatch: expected {expected}, got {actual}"
        )))
    }
}

fn require_field<T>(value: Option<T>, field: &str) -> io::Result<T> {
    value.ok_or_else(|| invalid_data(format!("audit report missing required field: {field}")))
}

fn parse_json_object_slice(s: &str) -> &str {
    let t = s.trim();
    if let (Some(i), Some(j)) = (t.find('{'), t.rfind('}')) {
        if j >= i {
            return &t[i..=j];
        }
    }
    t
}

fn validate_under_wiki_reports(wiki_dir: &Path, report_dir: &Path) -> io::Result<()> {
    let wiki_reports_dir = normalize_path(wiki_dir.join("reports"))?;
    fs::create_dir_all(&wiki_reports_dir)?;
    let canonical_wiki_reports_dir = fs::canonicalize(&wiki_reports_dir)?;
    let report_dir = normalize_path(report_dir)?;
    let nearest_existing = nearest_existing_ancestor(&report_dir)?;
    let canonical_nearest_existing = fs::canonicalize(nearest_existing)?;
    if canonical_nearest_existing.starts_with(&canonical_wiki_reports_dir) {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "orphan governance reports must be written under {}; got {}",
            canonical_wiki_reports_dir.display(),
            report_dir.display()
        ),
    ))
}

fn nearest_existing_ancestor(path: &Path) -> io::Result<PathBuf> {
    let mut current = normalize_path(path)?;
    loop {
        if current.exists() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no existing ancestor for {}", path.display()),
            ));
        }
    }
}

fn normalize_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref();
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

fn normalize_rel_path(path: &str) -> Option<String> {
    let path = path.trim().replace('\\', "/");
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path.split('/').any(|part| part == ".." || part.is_empty())
    {
        return None;
    }
    Some(path.trim_end_matches('/').to_string())
}

fn relative_slash_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

fn filename_timestamp(timestamp: &str) -> String {
    timestamp.replace(':', "-")
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
