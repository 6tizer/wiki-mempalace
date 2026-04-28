use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use wiki_core::RawArtifact;
use wiki_storage::{canonical_notion_page_id, NotionPageIndexRecord};

use crate::notion_client::NotionPageArchiveState;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
}
