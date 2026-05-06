use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use wiki_core::{AuditOperation, AuditRecord, RawArtifact, SourceId};
use wiki_kernel::{LlmWikiEngine, NoopWikiHook};
use wiki_storage::{canonical_notion_page_id, NotionPageIndexRecord, SqliteRepository};

use crate::notion_client::NotionPageArchiveState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotionArchivedRetirementReport {
    pub version: u32,
    pub generated_at: String,
    pub mode: String,
    pub total_indexed_pages: usize,
    pub archived_candidates: usize,
    pub active_pages: usize,
    pub missing_sources: usize,
    pub fetch_errors: Vec<NotionArchivedRetirementFetchError>,
    pub candidates: Vec<NotionArchivedRetirementCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotionArchivedRetirementCandidate {
    pub action_type: String,
    pub db_id: String,
    pub notion_page_id: String,
    pub notion_api_page_id: String,
    pub source_id: String,
    pub source_uri: String,
    pub title: String,
    pub archived: bool,
    pub in_trash: bool,
    pub synced_at: String,
    pub reason: String,
    pub apply_safe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotionArchivedRetirementFetchError {
    pub db_id: String,
    pub notion_page_id: String,
    pub source_id: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotionArchivedRetirementReportFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NotionArchivedRetirementApplyReport {
    pub version: u32,
    pub generated_at: String,
    pub mode: String,
    pub plan_generated_at: String,
    pub actions_seen: usize,
    pub safe_candidates: usize,
    pub unsafe_candidates: usize,
    pub stale_candidates: usize,
    pub sources_planned: usize,
    pub sources_removed: usize,
    pub index_rows_planned: usize,
    pub index_rows_deleted: usize,
    pub vault_files_planned: usize,
    pub vault_files_deleted: usize,
    pub applied_source_ids: Vec<String>,
    pub applied_notion_page_ids: Vec<String>,
    pub vault_files: Vec<String>,
    pub errors: Vec<NotionArchivedRetirementApplyError>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NotionArchivedRetirementApplyError {
    pub source_id: String,
    pub notion_page_id: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotionArchivedRetirementApplyFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotionArchivedRetirementApplyPlan {
    pub report: NotionArchivedRetirementApplyReport,
    pub source_ids: Vec<SourceId>,
    pub notion_page_ids: Vec<String>,
}

pub fn build_notion_archived_retirement_report(
    indexes: &[NotionPageIndexRecord],
    sources: &[RawArtifact],
    states: &BTreeMap<String, NotionPageArchiveState>,
    fetch_errors: Vec<NotionArchivedRetirementFetchError>,
    generated_at: OffsetDateTime,
) -> NotionArchivedRetirementReport {
    let source_by_id: BTreeMap<String, &RawArtifact> = sources
        .iter()
        .map(|source| (source.id.0.to_string(), source))
        .collect();
    let mut candidates = Vec::new();
    let mut active_pages = 0;
    let mut missing_sources = 0;

    for record in indexes {
        let source_id = record.source_id.0.to_string();
        let source = source_by_id.get(&source_id).copied();
        if source.is_none() {
            missing_sources += 1;
        }

        let canonical_page_id = canonical_notion_page_id(&record.notion_page_id);
        let Some(state) = states.get(&canonical_page_id) else {
            continue;
        };
        if !state.archived && !state.in_trash {
            active_pages += 1;
            continue;
        }

        let (source_uri, title, apply_safe, reason) = match source {
            Some(source) => (
                source.uri.clone(),
                source_title(source),
                true,
                "Notion page is archived/in_trash; plan retires the DB source first, then lets Vault/Palace projections follow.".to_string(),
            ),
            None => (
                String::new(),
                String::new(),
                false,
                "Notion page is archived/in_trash, but the DB source row is missing; apply must skip until DB evidence is reconciled.".to_string(),
            ),
        };

        candidates.push(NotionArchivedRetirementCandidate {
            action_type: "retire_notion_source".to_string(),
            db_id: record.db_id.clone(),
            notion_page_id: canonical_page_id,
            notion_api_page_id: state.id.clone(),
            source_id,
            source_uri,
            title,
            archived: state.archived,
            in_trash: state.in_trash,
            synced_at: format_time(record.synced_at),
            reason,
            apply_safe,
        });
    }

    NotionArchivedRetirementReport {
        version: 1,
        generated_at: format_time(generated_at),
        mode: "dry_run".to_string(),
        total_indexed_pages: indexes.len(),
        archived_candidates: candidates.len(),
        active_pages,
        missing_sources,
        fetch_errors,
        candidates,
    }
}

pub fn write_notion_archived_retirement_report(
    report: &NotionArchivedRetirementReport,
    report_dir: &Path,
) -> Result<NotionArchivedRetirementReportFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let stem = format!(
        "notion-archived-retirement-plan-{}",
        filename_timestamp(report.generated_at.as_str())
    );
    let json_path = report_dir.join(format!("{stem}.json"));
    let markdown_path = report_dir.join(format!("{stem}.md"));
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    let json_name = json_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("notion-archived-retirement-plan.json");
    std::fs::write(&markdown_path, render_markdown(report, json_name))?;
    Ok(NotionArchivedRetirementReportFiles {
        json_path,
        markdown_path,
    })
}

pub fn build_notion_archived_retirement_apply_plan(
    plan: &NotionArchivedRetirementReport,
    sources: &[RawArtifact],
    apply: bool,
    generated_at: OffsetDateTime,
) -> NotionArchivedRetirementApplyPlan {
    let source_by_id: BTreeMap<String, &RawArtifact> = sources
        .iter()
        .map(|source| (source.id.0.to_string(), source))
        .collect();
    let mut source_ids = Vec::new();
    let mut notion_page_ids = Vec::new();
    let mut seen_sources = BTreeSet::new();
    let mut seen_pages = BTreeSet::new();
    let mut errors = Vec::new();
    let mut unsafe_candidates = 0;
    let mut safe_candidates = 0;
    let mut stale_candidates = 0;

    for candidate in &plan.candidates {
        if !candidate.apply_safe {
            unsafe_candidates += 1;
            continue;
        }
        safe_candidates += 1;
        match validate_apply_candidate(candidate, &source_by_id) {
            Ok((source_id, notion_page_id)) => {
                let source_key = source_id.0.to_string();
                if seen_sources.insert(source_key) && seen_pages.insert(notion_page_id.clone()) {
                    source_ids.push(source_id);
                    notion_page_ids.push(notion_page_id);
                }
            }
            Err(error) => {
                stale_candidates += 1;
                errors.push(error);
            }
        }
    }

    NotionArchivedRetirementApplyPlan {
        report: NotionArchivedRetirementApplyReport {
            version: 1,
            generated_at: format_time(generated_at),
            mode: if apply { "apply" } else { "dry_run" }.to_string(),
            plan_generated_at: plan.generated_at.clone(),
            actions_seen: plan.candidates.len(),
            safe_candidates,
            unsafe_candidates,
            stale_candidates,
            sources_planned: source_ids.len(),
            sources_removed: 0,
            index_rows_planned: notion_page_ids.len(),
            index_rows_deleted: 0,
            vault_files_planned: 0,
            vault_files_deleted: 0,
            applied_source_ids: Vec::new(),
            applied_notion_page_ids: Vec::new(),
            vault_files: Vec::new(),
            errors,
        },
        source_ids,
        notion_page_ids,
    }
}

pub fn read_notion_archived_retirement_report(
    path: &Path,
) -> Result<NotionArchivedRetirementReport, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn collect_retired_source_files(
    vault: &Path,
    source_ids: &[SourceId],
    notion_page_ids: &[String],
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let source_ids: BTreeSet<String> = source_ids.iter().map(|id| id.0.to_string()).collect();
    let notion_page_ids: BTreeSet<String> = notion_page_ids
        .iter()
        .map(|id| canonical_notion_page_id(id))
        .collect();
    let root = vault.join("sources");
    let mut paths = BTreeSet::new();
    if !root.exists() {
        return Ok(Vec::new());
    }

    for entry in walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(path)?;
        let Some(frontmatter) = split_frontmatter(&text) else {
            continue;
        };
        let values = parse_frontmatter(frontmatter);
        let matches_source = values
            .get("source_id")
            .is_some_and(|source_id| source_ids.contains(source_id));
        let matches_notion = values
            .get("notion_uuid")
            .map(|notion_uuid| canonical_notion_page_id(notion_uuid))
            .is_some_and(|notion_uuid| notion_page_ids.contains(&notion_uuid));
        if matches_source || matches_notion {
            paths.insert(path.to_path_buf());
        }
    }

    Ok(paths.into_iter().collect())
}

pub fn delete_retired_source_files(paths: &[PathBuf]) -> Result<usize, Box<dyn std::error::Error>> {
    let mut deleted = 0;
    for path in paths {
        std::fs::remove_file(path)?;
        deleted += 1;
    }
    Ok(deleted)
}

pub fn apply_retirement(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    apply_plan: &mut NotionArchivedRetirementApplyPlan,
    vault_files: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    for source_id in &apply_plan.source_ids {
        eng.store.sources.remove(source_id);
        eng.audits.push(AuditRecord::new(
            AuditOperation::RetireSource,
            "notion-archived-retirement",
            format!("retired notion source {}", source_id.0),
        ));
    }
    let snapshot = eng.store.to_snapshot(&eng.audits);
    let deleted_index_rows =
        repo.save_snapshot_and_delete_notion_page_indexes(&snapshot, &apply_plan.notion_page_ids)?;
    let deleted_vault_files = delete_retired_source_files(vault_files)?;
    apply_plan.report.sources_removed = apply_plan.source_ids.len();
    apply_plan.report.index_rows_deleted = deleted_index_rows;
    apply_plan.report.vault_files_deleted = deleted_vault_files;
    apply_plan.report.applied_source_ids = apply_plan
        .source_ids
        .iter()
        .map(|source_id| source_id.0.to_string())
        .collect();
    apply_plan.report.applied_notion_page_ids = apply_plan.notion_page_ids.clone();
    Ok(())
}

pub fn write_notion_archived_retirement_apply_report(
    report: &NotionArchivedRetirementApplyReport,
    report_dir: &Path,
) -> Result<NotionArchivedRetirementApplyFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let stem = format!(
        "notion-archived-retirement-apply-{}",
        filename_timestamp(report.generated_at.as_str())
    );
    let json_path = report_dir.join(format!("{stem}.json"));
    let markdown_path = report_dir.join(format!("{stem}.md"));
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    let json_name = json_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("notion-archived-retirement-apply.json");
    std::fs::write(&markdown_path, render_apply_markdown(report, json_name))?;
    Ok(NotionArchivedRetirementApplyFiles {
        json_path,
        markdown_path,
    })
}

fn validate_apply_candidate(
    candidate: &NotionArchivedRetirementCandidate,
    source_by_id: &BTreeMap<String, &RawArtifact>,
) -> Result<(SourceId, String), NotionArchivedRetirementApplyError> {
    if candidate.action_type != "retire_notion_source" {
        return Err(apply_error(candidate, "unsupported action_type"));
    }
    let source_uuid = uuid::Uuid::parse_str(&candidate.source_id)
        .map_err(|err| apply_error(candidate, &format!("invalid source_id: {err}")))?;
    let source_id = SourceId(source_uuid);
    let Some(source) = source_by_id.get(&candidate.source_id).copied() else {
        return Err(apply_error(
            candidate,
            "source_id no longer exists in DB snapshot",
        ));
    };
    if !candidate.source_uri.is_empty() && source.uri != candidate.source_uri {
        return Err(apply_error(
            candidate,
            "source_uri changed since plan generation",
        ));
    }
    let Some((db_id, notion_page_id)) = parse_notion_uri(&source.uri) else {
        return Err(apply_error(candidate, "source uri is not notion://db/page"));
    };
    if db_id != candidate.db_id {
        return Err(apply_error(
            candidate,
            "source db_id changed since plan generation",
        ));
    }
    let notion_page_id = canonical_notion_page_id(notion_page_id);
    if notion_page_id != canonical_notion_page_id(&candidate.notion_page_id) {
        return Err(apply_error(
            candidate,
            "source notion page id changed since plan generation",
        ));
    }
    Ok((source_id, notion_page_id))
}

fn apply_error(
    candidate: &NotionArchivedRetirementCandidate,
    error: &str,
) -> NotionArchivedRetirementApplyError {
    NotionArchivedRetirementApplyError {
        source_id: candidate.source_id.clone(),
        notion_page_id: candidate.notion_page_id.clone(),
        error: error.to_string(),
    }
}

fn source_title(source: &RawArtifact) -> String {
    for line in source.body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(title) = trimmed.strip_prefix("# ") {
            let title = title.trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
        return trimmed.chars().take(120).collect();
    }
    source.uri.clone()
}

fn render_markdown(report: &NotionArchivedRetirementReport, sibling_json: &str) -> String {
    let mut out = String::new();
    out.push_str("# Notion Archived Source Retirement Plan\n\n");
    out.push_str(&format!("- generated_at: `{}`\n", report.generated_at));
    out.push_str(&format!("- mode: `{}`\n", report.mode));
    out.push_str(&format!("- source_json: `{sibling_json}`\n"));
    out.push_str(&format!(
        "- total_indexed_pages: `{}`\n- archived_candidates: `{}`\n- active_pages: `{}`\n- missing_sources: `{}`\n- fetch_errors: `{}`\n\n",
        report.total_indexed_pages,
        report.archived_candidates,
        report.active_pages,
        report.missing_sources,
        report.fetch_errors.len()
    ));

    out.push_str("## Candidates\n\n");
    if report.candidates.is_empty() {
        out.push_str("No archived Notion source retirement candidates.\n\n");
    } else {
        out.push_str("| db | notion_page_id | source_id | apply_safe | title |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for candidate in &report.candidates {
            out.push_str(&format!(
                "| {} | `{}` | `{}` | `{}` | {} |\n",
                md_cell(&candidate.db_id),
                candidate.notion_page_id,
                candidate.source_id,
                candidate.apply_safe,
                md_cell(&candidate.title)
            ));
        }
        out.push('\n');
    }

    if !report.fetch_errors.is_empty() {
        out.push_str("## Fetch Errors\n\n");
        out.push_str("| db | notion_page_id | source_id | error |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for error in &report.fetch_errors {
            out.push_str(&format!(
                "| {} | `{}` | `{}` | {} |\n",
                md_cell(&error.db_id),
                error.notion_page_id,
                error.source_id,
                md_cell(&error.error)
            ));
        }
        out.push('\n');
    }

    out.push_str("## Apply Boundary\n\n");
    out.push_str(
        "This report is dry-run only. Apply must retire DB source origin first; Vault and Mempalace update through projection/consumer paths.\n",
    );
    out
}

fn render_apply_markdown(
    report: &NotionArchivedRetirementApplyReport,
    sibling_json: &str,
) -> String {
    let mut out = String::new();
    out.push_str("# Notion Archived Source Retirement Apply\n\n");
    out.push_str(&format!("- generated_at: `{}`\n", report.generated_at));
    out.push_str(&format!("- mode: `{}`\n", report.mode));
    out.push_str(&format!(
        "- plan_generated_at: `{}`\n",
        report.plan_generated_at
    ));
    out.push_str(&format!("- source_json: `{sibling_json}`\n"));
    out.push_str(&format!(
        "- actions_seen: `{}`\n- safe_candidates: `{}`\n- unsafe_candidates: `{}`\n- stale_candidates: `{}`\n- sources_planned: `{}`\n- sources_removed: `{}`\n- index_rows_deleted: `{}`\n- vault_files_deleted: `{}`\n\n",
        report.actions_seen,
        report.safe_candidates,
        report.unsafe_candidates,
        report.stale_candidates,
        report.sources_planned,
        report.sources_removed,
        report.index_rows_deleted,
        report.vault_files_deleted
    ));

    if !report.applied_source_ids.is_empty() {
        out.push_str("## Applied Sources\n\n");
        for source_id in &report.applied_source_ids {
            out.push_str(&format!("- `{source_id}`\n"));
        }
        out.push('\n');
    }

    if !report.vault_files.is_empty() {
        out.push_str("## Vault Files\n\n");
        for path in &report.vault_files {
            out.push_str(&format!("- `{}`\n", path.replace('`', "\\`")));
        }
        out.push('\n');
    }

    if !report.errors.is_empty() {
        out.push_str("## Errors\n\n");
        out.push_str("| source_id | notion_page_id | error |\n");
        out.push_str("| --- | --- | --- |\n");
        for error in &report.errors {
            out.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                error.source_id,
                error.notion_page_id,
                md_cell(&error.error)
            ));
        }
        out.push('\n');
    }

    out
}

fn parse_notion_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix("notion://")?;
    rest.split_once('/')
}

fn split_frontmatter(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---\n")?;
    let idx = rest.find("\n---")?;
    Some(&rest[..idx])
}

fn parse_frontmatter(frontmatter: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in frontmatter.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        values.insert(key.trim().to_string(), unquote(value.trim()));
    }
    values
}

fn unquote(value: &str) -> String {
    value
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn md_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn format_time(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}

fn filename_timestamp(value: &str) -> String {
    value.chars().filter(|ch| *ch != ':').collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{RawArtifact, Scope, SourceId};

    fn source(id: SourceId, uri: &str, body: &str) -> RawArtifact {
        let mut source = RawArtifact::new(
            uri,
            body,
            Scope::Shared {
                team_id: "wiki".to_string(),
            },
        );
        source.id = id;
        source
    }

    #[test]
    fn report_builds_archived_candidates_without_mutating_sources() {
        let source_id =
            SourceId(uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap());
        let missing_source_id =
            SourceId(uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap());
        let indexes = vec![
            NotionPageIndexRecord {
                notion_page_id: "AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA".to_string(),
                db_id: "wechat".to_string(),
                source_id,
                synced_at: OffsetDateTime::UNIX_EPOCH,
            },
            NotionPageIndexRecord {
                notion_page_id: "BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB".to_string(),
                db_id: "x_bookmark".to_string(),
                source_id: missing_source_id,
                synced_at: OffsetDateTime::UNIX_EPOCH,
            },
            NotionPageIndexRecord {
                notion_page_id: "CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC".to_string(),
                db_id: "wechat".to_string(),
                source_id,
                synced_at: OffsetDateTime::UNIX_EPOCH,
            },
        ];
        let sources = vec![source(
            source_id,
            "notion://wechat/a",
            "# Archived title\nbody",
        )];
        let states = BTreeMap::from([
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                NotionPageArchiveState {
                    id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string(),
                    archived: true,
                    in_trash: false,
                },
            ),
            (
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                NotionPageArchiveState {
                    id: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb".to_string(),
                    archived: false,
                    in_trash: true,
                },
            ),
            (
                "cccccccccccccccccccccccccccccccc".to_string(),
                NotionPageArchiveState {
                    id: "cccccccc-cccc-cccc-cccc-cccccccccccc".to_string(),
                    archived: false,
                    in_trash: false,
                },
            ),
        ]);

        let report = build_notion_archived_retirement_report(
            &indexes,
            &sources,
            &states,
            Vec::new(),
            OffsetDateTime::UNIX_EPOCH,
        );

        assert_eq!(report.total_indexed_pages, 3);
        assert_eq!(report.archived_candidates, 2);
        assert_eq!(report.active_pages, 1);
        assert_eq!(report.missing_sources, 1);
        assert_eq!(report.candidates[0].title, "Archived title");
        assert!(report.candidates[0].apply_safe);
        assert!(!report.candidates[1].apply_safe);
    }

    #[test]
    fn apply_plan_filters_unsafe_and_stale_candidates() {
        let source_id =
            SourceId(uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap());
        let missing_id =
            SourceId(uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap());
        let sources = vec![source(
            source_id,
            "notion://wechat/AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA",
            "# Archived title\nbody",
        )];
        let plan = NotionArchivedRetirementReport {
            version: 1,
            generated_at: "1970-01-01T00:00:00Z".to_string(),
            mode: "dry_run".to_string(),
            total_indexed_pages: 3,
            archived_candidates: 3,
            active_pages: 0,
            missing_sources: 0,
            fetch_errors: Vec::new(),
            candidates: vec![
                NotionArchivedRetirementCandidate {
                    action_type: "retire_notion_source".to_string(),
                    db_id: "wechat".to_string(),
                    notion_page_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    notion_api_page_id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string(),
                    source_id: source_id.0.to_string(),
                    source_uri: "notion://wechat/AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA".to_string(),
                    title: "Archived title".to_string(),
                    archived: true,
                    in_trash: false,
                    synced_at: "1970-01-01T00:00:00Z".to_string(),
                    reason: "test".to_string(),
                    apply_safe: true,
                },
                NotionArchivedRetirementCandidate {
                    action_type: "retire_notion_source".to_string(),
                    db_id: "wechat".to_string(),
                    notion_page_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                    notion_api_page_id: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb".to_string(),
                    source_id: missing_id.0.to_string(),
                    source_uri: "notion://wechat/BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB".to_string(),
                    title: "Missing".to_string(),
                    archived: true,
                    in_trash: false,
                    synced_at: "1970-01-01T00:00:00Z".to_string(),
                    reason: "test".to_string(),
                    apply_safe: true,
                },
                NotionArchivedRetirementCandidate {
                    action_type: "retire_notion_source".to_string(),
                    db_id: "wechat".to_string(),
                    notion_page_id: "cccccccccccccccccccccccccccccccc".to_string(),
                    notion_api_page_id: "cccccccc-cccc-cccc-cccc-cccccccccccc".to_string(),
                    source_id: missing_id.0.to_string(),
                    source_uri: String::new(),
                    title: String::new(),
                    archived: true,
                    in_trash: false,
                    synced_at: "1970-01-01T00:00:00Z".to_string(),
                    reason: "test".to_string(),
                    apply_safe: false,
                },
            ],
        };

        let apply_plan = build_notion_archived_retirement_apply_plan(
            &plan,
            &sources,
            true,
            OffsetDateTime::UNIX_EPOCH,
        );

        assert_eq!(apply_plan.source_ids, vec![source_id]);
        assert_eq!(
            apply_plan.notion_page_ids,
            vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()]
        );
        assert_eq!(apply_plan.report.safe_candidates, 2);
        assert_eq!(apply_plan.report.unsafe_candidates, 1);
        assert_eq!(apply_plan.report.stale_candidates, 1);
        assert_eq!(apply_plan.report.sources_planned, 1);
    }

    #[test]
    fn collect_retired_source_files_matches_frontmatter_identity() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path();
        let retired = vault.join("sources/wechat/retired.md");
        let active = vault.join("sources/wechat/active.md");
        write_file(
            &retired,
            "---\nsource_id: \"11111111-1111-1111-1111-111111111111\"\nnotion_uuid: \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n---\n\nbody\n",
        );
        write_file(
            &active,
            "---\nsource_id: \"22222222-2222-2222-2222-222222222222\"\nnotion_uuid: \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"\n---\n\nbody\n",
        );

        let paths = collect_retired_source_files(
            vault,
            &[SourceId(
                uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            )],
            &["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_string()],
        )
        .unwrap();

        assert_eq!(paths, vec![retired.clone()]);
        assert_eq!(delete_retired_source_files(&paths).unwrap(), 1);
        assert!(!retired.exists());
        assert!(active.exists());
    }

    fn write_file(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }
}
