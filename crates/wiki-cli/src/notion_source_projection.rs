use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use time::format_description::well_known::Rfc3339;
use wiki_core::RawArtifact;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    DryRun,
    Apply,
}

impl ProjectionMode {
    fn as_str(self) -> &'static str {
        match self {
            ProjectionMode::DryRun => "dry_run",
            ProjectionMode::Apply => "apply",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotionSourceProjectionReport {
    pub mode: String,
    pub notion_sources_seen: usize,
    pub planned: usize,
    pub applied: usize,
    pub existing: usize,
    pub skipped_unknown_db: usize,
}

impl fmt::Display for NotionSourceProjectionReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "notion_source_vault_sync mode={} notion_sources_seen={} planned={} applied={} existing={} skipped_unknown_db={}",
            self.mode,
            self.notion_sources_seen,
            self.planned,
            self.applied,
            self.existing,
            self.skipped_unknown_db
        )
    }
}

#[derive(Debug, Clone)]
pub struct ObsidianTagRepairReport {
    pub mode: String,
    pub files_seen: usize,
    pub files_planned: usize,
    pub files_applied: usize,
    pub tags_rewritten: usize,
}

impl fmt::Display for ObsidianTagRepairReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "obsidian_tag_repair mode={} files_seen={} files_planned={} files_applied={} tags_rewritten={}",
            self.mode, self.files_seen, self.files_planned, self.files_applied, self.tags_rewritten
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NotionSourceProjectionError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("time format: {0}")]
    Time(#[from] time::error::Format),
}

#[derive(Debug, Clone)]
struct Projection {
    path: PathBuf,
    markdown: String,
}

pub fn project_notion_sources_to_vault(
    sources: &[RawArtifact],
    vault: &Path,
    mode: ProjectionMode,
) -> Result<NotionSourceProjectionReport, NotionSourceProjectionError> {
    let mut report = NotionSourceProjectionReport {
        mode: mode.as_str().to_string(),
        notion_sources_seen: 0,
        planned: 0,
        applied: 0,
        existing: 0,
        skipped_unknown_db: 0,
    };
    let existing = scan_existing_source_identity(vault)?;
    let mut seen_source_ids: BTreeSet<String> = existing.source_ids.keys().cloned().collect();
    let mut seen_notion_uuids: BTreeSet<String> = existing.notion_uuids.keys().cloned().collect();
    let mut planned_paths = BTreeSet::new();
    let mut projections = Vec::new();

    for source in sources {
        let Some((db_id, notion_uuid)) = parse_notion_uri(&source.uri) else {
            continue;
        };
        report.notion_sources_seen += 1;
        let Some(origin) = origin_from_db_id(db_id) else {
            report.skipped_unknown_db += 1;
            continue;
        };
        let source_id = source.id.0.to_string();
        let notion_uuid_key = normalize_notion_uuid(notion_uuid);
        if seen_source_ids.contains(&source_id) || seen_notion_uuids.contains(&notion_uuid_key) {
            report.existing += 1;
            continue;
        }
        let title = source_title(source);
        let path = unique_source_path(vault, origin, &title, &source_id, &planned_paths);
        planned_paths.insert(path.clone());
        let markdown = render_source_markdown(source, origin, notion_uuid, &title)?;
        report.planned += 1;
        seen_source_ids.insert(source_id);
        seen_notion_uuids.insert(notion_uuid_key);
        projections.push(Projection { path, markdown });
    }

    if mode == ProjectionMode::Apply {
        for projection in &projections {
            if let Some(parent) = projection.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&projection.path, &projection.markdown)?;
            report.applied += 1;
        }
    }

    Ok(report)
}

pub fn repair_obsidian_source_tags(
    vault: &Path,
    mode: ProjectionMode,
) -> Result<ObsidianTagRepairReport, NotionSourceProjectionError> {
    let root = vault.join("sources");
    let mut report = ObsidianTagRepairReport {
        mode: mode.as_str().to_string(),
        files_seen: 0,
        files_planned: 0,
        files_applied: 0,
        tags_rewritten: 0,
    };
    if !root.exists() {
        return Ok(report);
    }

    let mut repairs: Vec<(PathBuf, String, usize)> = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        report.files_seen += 1;
        let text = std::fs::read_to_string(path)?;
        if let Some((rewritten, changed)) = rewrite_frontmatter_tags(&text) {
            if changed > 0 {
                report.files_planned += 1;
                report.tags_rewritten += changed;
                repairs.push((path.to_path_buf(), rewritten, changed));
            }
        }
    }

    if mode == ProjectionMode::Apply {
        for (path, text, _) in repairs {
            std::fs::write(path, text)?;
            report.files_applied += 1;
        }
    }

    Ok(report)
}

#[derive(Debug, Default)]
struct ExistingSourceIdentity {
    source_ids: BTreeMap<String, PathBuf>,
    notion_uuids: BTreeMap<String, PathBuf>,
}

fn scan_existing_source_identity(vault: &Path) -> Result<ExistingSourceIdentity, std::io::Error> {
    let root = vault.join("sources");
    let mut existing = ExistingSourceIdentity::default();
    if !root.exists() {
        return Ok(existing);
    }
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(path)?;
        if let Some(frontmatter) = split_frontmatter(&text) {
            let values = parse_frontmatter(frontmatter);
            if let Some(source_id) = values.get("source_id") {
                existing
                    .source_ids
                    .insert(source_id.to_string(), path.to_path_buf());
            }
            if let Some(notion_uuid) = values.get("notion_uuid") {
                existing
                    .notion_uuids
                    .insert(normalize_notion_uuid(notion_uuid), path.to_path_buf());
            }
        }
    }
    Ok(existing)
}

fn parse_notion_uri(uri: &str) -> Option<(&str, &str)> {
    let rest = uri.strip_prefix("notion://")?;
    let (db_id, notion_uuid) = rest.split_once('/')?;
    if db_id.is_empty() || notion_uuid.is_empty() {
        return None;
    }
    Some((db_id, notion_uuid))
}

fn normalize_notion_uuid(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .flat_map(char::to_lowercase)
        .collect()
}

fn origin_from_db_id(db_id: &str) -> Option<&'static str> {
    match db_id {
        "x_bookmark" => Some("x"),
        "wechat" => Some("wechat"),
        _ => None,
    }
}

fn source_title(source: &RawArtifact) -> String {
    let first = source.body.lines().next().unwrap_or("").trim();
    let title = first.strip_prefix("# ").unwrap_or(first).trim();
    if title.is_empty() {
        source.uri.clone()
    } else {
        title.to_string()
    }
}

fn source_url(source: &RawArtifact) -> String {
    source
        .body
        .lines()
        .find_map(|line| line.trim().strip_prefix("URL: ").map(str::trim))
        .unwrap_or("")
        .to_string()
}

fn origin_label(source: &RawArtifact, origin: &str) -> String {
    source
        .body
        .lines()
        .find_map(|line| line.trim().strip_prefix("来源: ").map(str::trim))
        .filter(|value| !value.is_empty())
        .unwrap_or(match origin {
            "x" => "X",
            "wechat" => "微信",
            _ => origin,
        })
        .to_string()
}

fn unique_source_path(
    vault: &Path,
    origin: &str,
    title: &str,
    source_id: &str,
    planned_paths: &BTreeSet<PathBuf>,
) -> PathBuf {
    let dir = vault.join("sources").join(origin);
    let stem = vault_source_filename(title);
    let primary = dir.join(format!("{stem}.md"));
    if !primary.exists() && !planned_paths.contains(&primary) {
        return primary;
    }
    let short = source_id.chars().take(8).collect::<String>();
    dir.join(format!("{stem}-{short}.md"))
}

fn vault_source_filename(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut last_sep = true;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() || (!c.is_ascii() && !c.is_control()) || matches!(c, '-' | '_')
        {
            out.push(c);
            last_sep = false;
        } else if !last_sep {
            out.push('-');
            last_sep = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let filename = trimmed.chars().take(80).collect::<String>();
    if filename.is_empty() {
        "untitled-source".to_string()
    } else {
        filename
    }
}

fn render_source_markdown(
    source: &RawArtifact,
    origin: &str,
    notion_uuid: &str,
    title: &str,
) -> Result<String, time::error::Format> {
    let url = source_url(source);
    let origin_label = origin_label(source, origin);
    let created_at = source.ingested_at.format(&Rfc3339)?;
    let mut out = String::from("---\n");
    out.push_str(&format!("title: \"{}\"\n", yaml_escape(title)));
    out.push_str("kind: source\n");
    out.push_str(&format!("origin: {origin}\n"));
    out.push_str(&format!("url: \"{}\"\n", yaml_escape(&url)));
    out.push_str(&format!(
        "origin_label: \"{}\"\n",
        yaml_escape(&origin_label)
    ));
    out.push_str("published_at: \"\"\n");
    out.push_str("notes: \"\"\n");
    out.push_str("compiled_to_wiki: false\n");
    out.push_str(&format!("created_at: \"{}\"\n", yaml_escape(&created_at)));
    out.push_str(&format!("source_id: \"{}\"\n", source.id.0));
    out.push_str(&format!("notion_uuid: \"{}\"\n", yaml_escape(notion_uuid)));
    out.push_str(&yaml_string_list_block("tags", &source.tags));
    out.push_str("---\n\n");
    out.push_str(source.body.trim());
    out.push('\n');
    Ok(out)
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

fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn yaml_string_list_block(name: &str, items: &[String]) -> String {
    if items.is_empty() {
        format!("{name}: []\n")
    } else {
        let mut out = format!("{name}:\n");
        for item in items {
            out.push_str(&format!(
                "  - \"{}\"\n",
                yaml_escape(&obsidian_safe_tag(item))
            ));
        }
        out
    }
}

fn obsidian_safe_tag(tag: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    let mut last_sep = true;
    for c in tag.trim().chars() {
        if c.is_ascii_alphanumeric()
            || (!c.is_ascii() && !c.is_control() && !c.is_whitespace())
            || matches!(c, '-' | '_' | '/')
        {
            out.push(c);
            last_sep = false;
        } else if !last_sep {
            out.push('-');
            last_sep = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "untagged".to_string()
    } else {
        trimmed.to_string()
    }
}

fn rewrite_frontmatter_tags(text: &str) -> Option<(String, usize)> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];
    let body = &rest[end..];
    let mut changed = 0usize;
    let mut out = String::with_capacity(frontmatter.len());
    let mut lines = frontmatter.lines().peekable();

    while let Some(line) = lines.next() {
        if let Some(raw_value) = line.strip_prefix("tags:") {
            if let Some(tags) = parse_inline_tags(raw_value.trim()) {
                let safe_tags = safe_tags_and_count(&tags, &mut changed);
                out.push_str(&yaml_string_list_block("tags", &safe_tags));
                continue;
            }
            if raw_value.trim().is_empty() {
                let mut tags = Vec::new();
                while let Some(next) = lines.peek() {
                    if let Some(value) = next.trim_start().strip_prefix("- ") {
                        tags.push(unquote(value.trim()));
                        lines.next();
                    } else {
                        break;
                    }
                }
                let safe_tags = safe_tags_and_count(&tags, &mut changed);
                out.push_str(&yaml_string_list_block("tags", &safe_tags));
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    Some((format!("---\n{out}{body}"), changed))
}

fn safe_tags_and_count(tags: &[String], changed: &mut usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut safe_tags = Vec::new();
    for tag in tags {
        let safe = obsidian_safe_tag(tag);
        if &safe != tag {
            *changed += 1;
        }
        if seen.insert(safe.clone()) {
            safe_tags.push(safe);
        }
    }
    safe_tags
}

fn parse_inline_tags(value: &str) -> Option<Vec<String>> {
    let inner = value.strip_prefix('[')?.strip_suffix(']')?.trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    Some(
        inner
            .split(',')
            .map(|item| unquote(item.trim()))
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;
    use wiki_core::{RawArtifact, Scope};

    fn source(id: &str, uri: &str, body: &str) -> RawArtifact {
        let mut artifact = RawArtifact::new(
            uri,
            body,
            Scope::Shared {
                team_id: "wiki".into(),
            },
        );
        artifact.id = wiki_core::SourceId(uuid::Uuid::parse_str(id).unwrap());
        artifact.ingested_at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        artifact.tags = vec!["Agent".into(), "长期记忆".into()];
        artifact
    }

    #[test]
    fn dry_run_plans_missing_notion_source_without_writing() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let sources = vec![source(
            "11111111-1111-1111-1111-111111111111",
            "notion://wechat/abc-def",
            "# 标题 / One\n\nURL: https://example.com\n来源: 微信\n",
        )];

        let report =
            project_notion_sources_to_vault(&sources, &vault, ProjectionMode::DryRun).unwrap();

        assert_eq!(report.notion_sources_seen, 1);
        assert_eq!(report.planned, 1);
        assert_eq!(report.applied, 0);
        assert!(!vault.join("sources/wechat/标题-One.md").exists());
    }

    #[test]
    fn apply_writes_standard_source_file_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let sources = vec![source(
            "22222222-2222-2222-2222-222222222222",
            "notion://x_bookmark/abc-def",
            "# X Source\n\nURL: https://x.com/post\n来源: X\n",
        )];

        let first =
            project_notion_sources_to_vault(&sources, &vault, ProjectionMode::Apply).unwrap();
        assert_eq!(first.applied, 1);
        let path = vault.join("sources/x/X-Source.md");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("source_id: \"22222222-2222-2222-2222-222222222222\""));
        assert!(text.contains("notion_uuid: \"abc-def\""));
        assert!(text.contains("origin: x"));
        assert!(text.contains("compiled_to_wiki: false"));
        assert!(text.contains("  - \"Agent\""));

        let second =
            project_notion_sources_to_vault(&sources, &vault, ProjectionMode::Apply).unwrap();
        assert_eq!(second.planned, 0);
        assert_eq!(second.existing, 1);
    }

    #[test]
    fn existing_hyphenless_notion_uuid_matches_hyphenated_uri() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        std::fs::create_dir_all(vault.join("sources/x")).unwrap();
        std::fs::write(
            vault.join("sources/x/existing.md"),
            r#"---
notion_uuid: 1a9701074b688103b989fbd0cfb8343a
kind: source
---

body
"#,
        )
        .unwrap();
        let sources = vec![source(
            "33333333-3333-3333-3333-333333333333",
            "notion://x_bookmark/1a970107-4b68-8103-b989-fbd0cfb8343a",
            "# X Source\n\nURL: https://x.com/post\n来源: X\n",
        )];

        let report =
            project_notion_sources_to_vault(&sources, &vault, ProjectionMode::Apply).unwrap();

        assert_eq!(report.planned, 0);
        assert_eq!(report.applied, 0);
        assert_eq!(report.existing, 1);
        assert!(!vault.join("sources/x/X-Source.md").exists());
    }

    #[test]
    fn duplicate_notion_uuid_in_same_run_writes_once() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let sources = vec![
            source(
                "44444444-4444-4444-4444-444444444444",
                "notion://x_bookmark/1a970107-4b68-8103-b989-fbd0cfb8343a",
                "# X Source\n\nURL: https://x.com/post\n来源: X\n",
            ),
            source(
                "55555555-5555-5555-5555-555555555555",
                "notion://x_bookmark/1a9701074b688103b989fbd0cfb8343a",
                "# X Source Copy\n\nURL: https://x.com/post\n来源: X\n",
            ),
        ];

        let report =
            project_notion_sources_to_vault(&sources, &vault, ProjectionMode::Apply).unwrap();

        assert_eq!(report.notion_sources_seen, 2);
        assert_eq!(report.planned, 1);
        assert_eq!(report.applied, 1);
        assert_eq!(report.existing, 1);
        let files: Vec<_> = walkdir::WalkDir::new(vault.join("sources/x"))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
            .collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn projection_writes_obsidian_safe_tags() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let mut src = source(
            "66666666-6666-6666-6666-666666666666",
            "notion://x_bookmark/tag-test",
            "# X Source\n\nURL: https://x.com/post\n来源: X\n",
        );
        src.tags = vec![
            "Apache2.0".into(),
            "API Key".into(),
            "Apple Silicon".into(),
            "密码学/ZK".into(),
        ];

        project_notion_sources_to_vault(&[src], &vault, ProjectionMode::Apply).unwrap();

        let text = std::fs::read_to_string(vault.join("sources/x/X-Source.md")).unwrap();
        assert!(text.contains("  - \"Apache2-0\""));
        assert!(text.contains("  - \"API-Key\""));
        assert!(text.contains("  - \"Apple-Silicon\""));
        assert!(text.contains("  - \"密码学/ZK\""));
        assert!(!text.contains("Apache2.0"));
        assert!(!text.contains("API Key"));
    }

    #[test]
    fn repair_obsidian_source_tags_handles_inline_and_block_tags() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        std::fs::create_dir_all(vault.join("sources/x")).unwrap();
        let inline = vault.join("sources/x/inline.md");
        let block = vault.join("sources/x/block.md");
        std::fs::write(
            &inline,
            r#"---
title: Inline
tags: [API Key, Apple Silicon, 密码学/ZK]
---

body
"#,
        )
        .unwrap();
        std::fs::write(
            &block,
            r#"---
title: Block
tags:
  - "Apache2.0"
  - "Google Stitch"
---

body
"#,
        )
        .unwrap();

        let dry = repair_obsidian_source_tags(&vault, ProjectionMode::DryRun).unwrap();
        assert_eq!(dry.files_seen, 2);
        assert_eq!(dry.files_planned, 2);
        assert_eq!(dry.files_applied, 0);
        assert_eq!(dry.tags_rewritten, 4);

        let applied = repair_obsidian_source_tags(&vault, ProjectionMode::Apply).unwrap();
        assert_eq!(applied.files_applied, 2);
        let inline_text = std::fs::read_to_string(&inline).unwrap();
        let block_text = std::fs::read_to_string(&block).unwrap();
        assert!(inline_text.contains("  - \"API-Key\""));
        assert!(inline_text.contains("  - \"Apple-Silicon\""));
        assert!(inline_text.contains("  - \"密码学/ZK\""));
        assert!(block_text.contains("  - \"Apache2-0\""));
        assert!(block_text.contains("  - \"Google-Stitch\""));

        let again = repair_obsidian_source_tags(&vault, ProjectionMode::Apply).unwrap();
        assert_eq!(again.files_planned, 0);
        assert_eq!(again.files_applied, 0);
        assert_eq!(again.tags_rewritten, 0);
    }
}
