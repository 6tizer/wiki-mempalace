use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;
use wiki_core::SourceId;
use wiki_storage::{StorageError, WikiRepository};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackfillMode {
    DryRun,
    Apply,
}

impl BackfillMode {
    fn as_str(self) -> &'static str {
        match self {
            BackfillMode::DryRun => "dry_run",
            BackfillMode::Apply => "apply",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotionIndexBackfillReport {
    pub mode: String,
    pub sources_seen: usize,
    pub planned: usize,
    pub applied: usize,
    pub existing: usize,
    pub skipped_missing_notion_uuid: usize,
    pub skipped_missing_source_id: usize,
    pub skipped_unknown_origin: usize,
    pub skipped_invalid_source_id: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum NotionIndexBackfillError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
}

#[derive(Debug, Clone)]
struct Candidate {
    notion_uuid: String,
    db_id: String,
    source_id: SourceId,
}

pub fn backfill_notion_page_index<R: WikiRepository>(
    repo: &R,
    vault: &Path,
    mode: BackfillMode,
) -> Result<NotionIndexBackfillReport, NotionIndexBackfillError> {
    let mut report = NotionIndexBackfillReport {
        mode: mode.as_str().to_string(),
        sources_seen: 0,
        planned: 0,
        applied: 0,
        existing: 0,
        skipped_missing_notion_uuid: 0,
        skipped_missing_source_id: 0,
        skipped_unknown_origin: 0,
        skipped_invalid_source_id: 0,
    };
    let mut planned = Vec::new();
    let mut seen_notion_ids = BTreeSet::new();

    for path in source_markdown_paths(vault) {
        report.sources_seen += 1;
        let text = std::fs::read_to_string(&path)?;
        let Some(frontmatter) = split_frontmatter(&text) else {
            report.skipped_missing_notion_uuid += 1;
            continue;
        };
        let values = parse_frontmatter(frontmatter);
        let Some(notion_uuid) = values.get("notion_uuid").cloned() else {
            report.skipped_missing_notion_uuid += 1;
            continue;
        };
        let Some(source_id_raw) = values.get("source_id") else {
            report.skipped_missing_source_id += 1;
            continue;
        };
        let Ok(source_uuid) = uuid::Uuid::parse_str(source_id_raw) else {
            report.skipped_invalid_source_id += 1;
            continue;
        };
        let Some(db_id) = db_id_for_source(vault, &path, values.get("origin").map(String::as_str))
        else {
            report.skipped_unknown_origin += 1;
            continue;
        };
        if repo.notion_page_exists(&notion_uuid)?
            || !seen_notion_ids.insert(canonical(&notion_uuid))
        {
            report.existing += 1;
            continue;
        }
        report.planned += 1;
        planned.push(Candidate {
            notion_uuid,
            db_id: db_id.to_string(),
            source_id: SourceId(source_uuid),
        });
    }

    if mode == BackfillMode::Apply && !planned.is_empty() {
        let entries: Vec<_> = planned
            .into_iter()
            .map(|candidate| (candidate.notion_uuid, candidate.db_id, candidate.source_id))
            .collect();
        repo.insert_notion_page_indexes(&entries)?;
        report.applied = entries.len();
    }

    Ok(report)
}

fn source_markdown_paths(vault: &Path) -> Vec<PathBuf> {
    let root = vault.join("sources");
    if !root.exists() {
        return Vec::new();
    }
    let mut paths = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            paths.push(path.to_path_buf());
        }
    }
    paths.sort();
    paths
}

fn split_frontmatter(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---\n")?;
    let idx = rest.find("\n---")?;
    Some(&rest[..idx])
}

fn parse_frontmatter(frontmatter: &str) -> std::collections::BTreeMap<String, String> {
    let mut values = std::collections::BTreeMap::new();
    for line in frontmatter.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        values.insert(key.to_string(), unquote(value.trim()));
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

fn db_id_for_source(vault: &Path, path: &Path, origin: Option<&str>) -> Option<&'static str> {
    match origin.map(str::trim) {
        Some("x") => return Some("x_bookmark"),
        Some("wechat") => return Some("wechat"),
        _ => {}
    }
    let rel = path.strip_prefix(vault).ok()?;
    let mut parts = rel.components();
    let first = parts.next()?.as_os_str().to_str()?;
    let second = parts.next()?.as_os_str().to_str()?;
    if first != "sources" {
        return None;
    }
    match second {
        "x" => Some("x_bookmark"),
        "wechat" => Some("wechat"),
        _ => None,
    }
}

fn canonical(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| *ch != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

impl fmt::Display for NotionIndexBackfillReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "notion_sync_index_backfill mode={} sources_seen={} planned={} applied={} existing={} skipped_missing_notion_uuid={} skipped_missing_source_id={} skipped_unknown_origin={} skipped_invalid_source_id={}",
            self.mode,
            self.sources_seen,
            self.planned,
            self.applied,
            self.existing,
            self.skipped_missing_notion_uuid,
            self.skipped_missing_source_id,
            self.skipped_unknown_origin,
            self.skipped_invalid_source_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_storage::SqliteRepository;

    fn write_file(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn backfill_dry_run_does_not_insert() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let db = temp.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        write_file(
            &vault.join("sources/wechat/a.md"),
            "---\nsource_id: \"11111111-1111-1111-1111-111111111111\"\nnotion_uuid: 1a9701074b688103b989fbd0cfb8343a\norigin: wechat\n---\n\nbody\n",
        );

        let report = backfill_notion_page_index(&repo, &vault, BackfillMode::DryRun).unwrap();

        assert_eq!(report.sources_seen, 1);
        assert_eq!(report.planned, 1);
        assert_eq!(report.applied, 0);
        assert!(!repo
            .notion_page_exists("1a970107-4b68-8103-b989-fbd0cfb8343a")
            .unwrap());
    }

    #[test]
    fn backfill_apply_inserts_and_rerun_is_existing() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let db = temp.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        write_file(
            &vault.join("sources/x/a.md"),
            "---\nsource_id: \"22222222-2222-2222-2222-222222222222\"\nnotion_uuid: \"1A9701074B688103B989FBD0CFB8343A\"\n---\n\nbody\n",
        );

        let first = backfill_notion_page_index(&repo, &vault, BackfillMode::Apply).unwrap();
        assert_eq!(first.applied, 1);
        assert!(repo
            .notion_page_exists("1a970107-4b68-8103-b989-fbd0cfb8343a")
            .unwrap());

        let second = backfill_notion_page_index(&repo, &vault, BackfillMode::Apply).unwrap();
        assert_eq!(second.planned, 0);
        assert_eq!(second.existing, 1);
    }
}
