use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use walkdir::WalkDir;
use wiki_core::{
    EntryStatus, EntryType, PageId, RawArtifact, Scope, SourceId, WikiEvent, WikiPage,
};
use wiki_storage::{SqliteRepository, StorageError, WikiRepository};

pub type Result<T> = std::result::Result<T, VaultBackfillError>;

#[derive(Debug)]
pub enum VaultBackfillError {
    InvalidScope(String),
    InvalidUuid {
        path: PathBuf,
        field: &'static str,
        value: String,
    },
    Io(io::Error),
    Storage(StorageError),
    Json(serde_json::Error),
}

impl fmt::Display for VaultBackfillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScope(raw) => write!(f, "invalid scope: {raw}"),
            Self::InvalidUuid { path, field, value } => {
                write!(f, "invalid uuid in {}: {field}={value}", path.display())
            }
            Self::Io(err) => write!(f, "io: {err}"),
            Self::Storage(err) => write!(f, "storage: {err}"),
            Self::Json(err) => write!(f, "json: {err}"),
        }
    }
}

impl Error for VaultBackfillError {}

impl From<io::Error> for VaultBackfillError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<StorageError> for VaultBackfillError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<serde_json::Error> for VaultBackfillError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackfillMode {
    DryRun,
    Apply,
}

impl BackfillMode {
    fn is_apply(self) -> bool {
        matches!(self, Self::Apply)
    }
}

#[derive(Debug, Clone)]
pub struct VaultBackfillOptions {
    pub vault_path: PathBuf,
    pub db_path: PathBuf,
    pub scope: Scope,
    pub mode: BackfillMode,
    pub limit: Option<usize>,
    pub report_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackfillReport {
    pub mode: String,
    pub vault_path: PathBuf,
    pub db_path: PathBuf,
    pub scope: Scope,
    pub sources_seen: usize,
    pub pages_seen: usize,
    pub source_id_writes_planned: usize,
    pub page_id_writes_planned: usize,
    pub source_id_writes_applied: usize,
    pub page_id_writes_applied: usize,
    pub sources_imported: usize,
    pub pages_imported: usize,
    pub sources_updated: usize,
    pub pages_updated: usize,
    pub page_written_events: usize,
    pub skipped: Vec<BackfillSkip>,
    pub records: Vec<BackfillRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackfillSkip {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackfillRecord {
    pub path: PathBuf,
    pub kind: String,
    pub id: String,
    pub id_was_missing: bool,
    pub imported: bool,
    pub updated: bool,
}

#[derive(Debug, Clone)]
pub struct VaultBackfillPlan {
    sources: Vec<PlannedSource>,
    pages: Vec<PlannedPage>,
    skipped: Vec<BackfillSkip>,
}

#[derive(Debug, Clone)]
struct PlannedSource {
    path: PathBuf,
    markdown: MarkdownDoc,
    source_id: SourceId,
    id_was_missing: bool,
    uri: String,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct PlannedPage {
    path: PathBuf,
    markdown: MarkdownDoc,
    page_id: PageId,
    id_was_missing: bool,
    title: String,
    entry_type: EntryType,
    status: EntryStatus,
}

#[derive(Debug, Clone)]
struct MarkdownDoc {
    frontmatter: String,
    body: String,
    values: HashMap<String, String>,
}

pub fn parse_scope(raw: &str) -> Result<Scope> {
    if let Some(agent_id) = raw.strip_prefix("private:") {
        if !agent_id.trim().is_empty() {
            return Ok(Scope::Private {
                agent_id: agent_id.trim().to_string(),
            });
        }
    }
    if let Some(team_id) = raw.strip_prefix("shared:") {
        if !team_id.trim().is_empty() {
            return Ok(Scope::Shared {
                team_id: team_id.trim().to_string(),
            });
        }
    }
    Err(VaultBackfillError::InvalidScope(raw.to_string()))
}

#[allow(dead_code)]
pub fn backfill_vault_with_scope_str(
    vault_path: impl Into<PathBuf>,
    db_path: impl Into<PathBuf>,
    scope: &str,
    mode: BackfillMode,
    limit: Option<usize>,
    report_dir: impl Into<PathBuf>,
) -> Result<BackfillReport> {
    backfill_vault(VaultBackfillOptions {
        vault_path: vault_path.into(),
        db_path: db_path.into(),
        scope: parse_scope(scope)?,
        mode,
        limit,
        report_dir: report_dir.into(),
    })
}

pub fn backfill_vault(options: VaultBackfillOptions) -> Result<BackfillReport> {
    let plan = plan_vault_backfill(&options.vault_path, options.limit)?;
    let mut report = empty_report(&options, &plan);

    if options.mode.is_apply() {
        apply_frontmatter_ids(&plan, &mut report)?;
        let repo = SqliteRepository::open(&options.db_path)?;
        apply_plan_to_repo(&plan, &repo, &options.scope, &mut report)?;
    }

    write_report_files(&options.report_dir, &report)?;
    Ok(report)
}

#[allow(dead_code)]
pub fn backfill_vault_with_repo<R: WikiRepository>(
    vault_path: impl AsRef<Path>,
    repo: &R,
    scope: Scope,
    mode: BackfillMode,
    limit: Option<usize>,
    report_dir: impl AsRef<Path>,
) -> Result<BackfillReport> {
    let vault_path = vault_path.as_ref();
    let plan = plan_vault_backfill(vault_path, limit)?;
    let options = VaultBackfillOptions {
        vault_path: vault_path.to_path_buf(),
        db_path: PathBuf::from("<repository>"),
        scope,
        mode,
        limit,
        report_dir: report_dir.as_ref().to_path_buf(),
    };
    let mut report = empty_report(&options, &plan);

    if mode.is_apply() {
        apply_frontmatter_ids(&plan, &mut report)?;
        apply_plan_to_repo(&plan, repo, &options.scope, &mut report)?;
    }

    write_report_files(&options.report_dir, &report)?;
    Ok(report)
}

pub fn plan_vault_backfill(vault_path: &Path, limit: Option<usize>) -> Result<VaultBackfillPlan> {
    let mut skipped = Vec::new();
    let mut sources = Vec::new();
    let mut pages = Vec::new();

    let source_paths = collect_markdown(vault_path, &["sources"])?;
    let page_paths = collect_markdown(vault_path, &["pages"])?;
    let max = limit.unwrap_or(usize::MAX);

    for path in source_paths.into_iter().take(max) {
        match plan_source(vault_path, path) {
            Ok(source) => sources.push(source),
            Err(skip) => skipped.push(skip),
        }
    }

    let remaining = max.saturating_sub(sources.len());
    for path in page_paths.into_iter().take(remaining) {
        match plan_page(vault_path, path) {
            Ok(page) => pages.push(page),
            Err(skip) => skipped.push(skip),
        }
    }
    reject_duplicate_planned_ids(&mut sources, &mut pages, &mut skipped);

    Ok(VaultBackfillPlan {
        sources,
        pages,
        skipped,
    })
}

fn reject_duplicate_planned_ids(
    sources: &mut Vec<PlannedSource>,
    pages: &mut Vec<PlannedPage>,
    skipped: &mut Vec<BackfillSkip>,
) {
    let mut source_counts: HashMap<uuid::Uuid, usize> = HashMap::new();
    for source in sources.iter() {
        *source_counts.entry(source.source_id.0).or_default() += 1;
    }
    let mut page_counts: HashMap<uuid::Uuid, usize> = HashMap::new();
    for page in pages.iter() {
        *page_counts.entry(page.page_id.0).or_default() += 1;
    }

    let mut rejected_sources = Vec::new();
    sources.retain(|source| {
        if source_counts.get(&source.source_id.0).copied().unwrap_or(0) > 1 {
            rejected_sources.push(BackfillSkip {
                path: source.path.clone(),
                reason: format!("duplicate source_id {}", source.source_id.0),
            });
            false
        } else {
            true
        }
    });

    let mut rejected_pages = Vec::new();
    pages.retain(|page| {
        if page_counts.get(&page.page_id.0).copied().unwrap_or(0) > 1 {
            rejected_pages.push(BackfillSkip {
                path: page.path.clone(),
                reason: format!("duplicate page_id {}", page.page_id.0),
            });
            false
        } else {
            true
        }
    });

    skipped.extend(rejected_sources);
    skipped.extend(rejected_pages);
}

fn empty_report(options: &VaultBackfillOptions, plan: &VaultBackfillPlan) -> BackfillReport {
    let mut records = Vec::with_capacity(plan.sources.len() + plan.pages.len());
    for source in &plan.sources {
        records.push(BackfillRecord {
            path: source.path.clone(),
            kind: "source".to_string(),
            id: source.source_id.0.to_string(),
            id_was_missing: source.id_was_missing,
            imported: false,
            updated: false,
        });
    }
    for page in &plan.pages {
        records.push(BackfillRecord {
            path: page.path.clone(),
            kind: "page".to_string(),
            id: page.page_id.0.to_string(),
            id_was_missing: page.id_was_missing,
            imported: false,
            updated: false,
        });
    }

    BackfillReport {
        mode: match options.mode {
            BackfillMode::DryRun => "dry_run".to_string(),
            BackfillMode::Apply => "apply".to_string(),
        },
        vault_path: options.vault_path.clone(),
        db_path: options.db_path.clone(),
        scope: options.scope.clone(),
        sources_seen: plan.sources.len(),
        pages_seen: plan.pages.len(),
        source_id_writes_planned: plan.sources.iter().filter(|s| s.id_was_missing).count(),
        page_id_writes_planned: plan.pages.iter().filter(|p| p.id_was_missing).count(),
        source_id_writes_applied: 0,
        page_id_writes_applied: 0,
        sources_imported: 0,
        pages_imported: 0,
        sources_updated: 0,
        pages_updated: 0,
        page_written_events: 0,
        skipped: plan.skipped.clone(),
        records,
    }
}

fn collect_markdown(vault_path: &Path, parts: &[&str]) -> Result<Vec<PathBuf>> {
    let mut root = vault_path.to_path_buf();
    for part in parts {
        root.push(part);
    }
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            paths.push(path.to_path_buf());
        }
    }
    paths.sort();
    Ok(paths)
}

fn plan_source(
    vault_path: &Path,
    path: PathBuf,
) -> std::result::Result<PlannedSource, BackfillSkip> {
    let rel_path = relative_slash_path(vault_path, &path);
    let markdown = parse_markdown_file(&path).map_err(|err| BackfillSkip {
        path: path.clone(),
        reason: err.to_string(),
    })?;
    let source_id = read_or_stable_id(&path, &markdown, "source_id", "source", &rel_path)
        .map(SourceId)
        .map_err(|err| BackfillSkip {
            path: path.clone(),
            reason: err.to_string(),
        })?;
    let uri = markdown
        .values
        .get("url")
        .filter(|url| !url.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("file://{rel_path}"));
    let tags = parse_tags(markdown.values.get("tags"));
    let id_was_missing = !has_non_empty(&markdown.values, "source_id");
    Ok(PlannedSource {
        path,
        markdown,
        source_id,
        id_was_missing,
        uri,
        tags,
    })
}

fn plan_page(vault_path: &Path, path: PathBuf) -> std::result::Result<PlannedPage, BackfillSkip> {
    let rel_path = relative_slash_path(vault_path, &path);
    let markdown = parse_markdown_file(&path).map_err(|err| BackfillSkip {
        path: path.clone(),
        reason: err.to_string(),
    })?;
    let entry_type = markdown
        .values
        .get("entry_type")
        .map(|raw| EntryType::parse(raw).map_err(|err| err.to_string()))
        .transpose()
        .map_err(|reason| BackfillSkip {
            path: path.clone(),
            reason,
        })?
        .unwrap_or_else(|| entry_type_from_path(&rel_path));
    let page_id = read_or_stable_id(
        &path,
        &markdown,
        "page_id",
        entry_type_name(&entry_type),
        &rel_path,
    )
    .map(PageId)
    .map_err(|err| BackfillSkip {
        path: path.clone(),
        reason: err.to_string(),
    })?;
    let title = markdown
        .values
        .get("title")
        .cloned()
        .unwrap_or_else(|| file_stem_title(&path));
    let status = markdown
        .values
        .get("status")
        .map(|raw| EntryStatus::parse(raw).map_err(|err| err.to_string()))
        .transpose()
        .map_err(|reason| BackfillSkip {
            path: path.clone(),
            reason,
        })?
        .unwrap_or(EntryStatus::Draft);
    let id_was_missing = !has_non_empty(&markdown.values, "page_id");
    Ok(PlannedPage {
        path,
        markdown,
        page_id,
        id_was_missing,
        title,
        entry_type,
        status,
    })
}

fn read_or_stable_id(
    path: &Path,
    markdown: &MarkdownDoc,
    field: &'static str,
    kind: &str,
    rel_path: &str,
) -> Result<Uuid> {
    if let Some(raw) = markdown
        .values
        .get(field)
        .filter(|raw| !raw.trim().is_empty())
    {
        return Uuid::parse_str(raw.trim()).map_err(|_| VaultBackfillError::InvalidUuid {
            path: path.to_path_buf(),
            field,
            value: raw.clone(),
        });
    }
    let notion = markdown
        .values
        .get("notion_uuid")
        .map(String::as_str)
        .unwrap_or("");
    Ok(stable_v5_uuid(kind, rel_path, notion))
}

fn apply_frontmatter_ids(plan: &VaultBackfillPlan, report: &mut BackfillReport) -> Result<()> {
    for source in &plan.sources {
        if !source.id_was_missing {
            continue;
        }
        write_doc_with_inserted_id(
            &source.path,
            &source.markdown,
            "source_id",
            source.source_id.0,
        )?;
        report.source_id_writes_applied += 1;
    }
    for page in &plan.pages {
        if !page.id_was_missing {
            continue;
        }
        write_doc_with_inserted_id(&page.path, &page.markdown, "page_id", page.page_id.0)?;
        report.page_id_writes_applied += 1;
    }
    Ok(())
}

fn apply_plan_to_repo<R: WikiRepository>(
    plan: &VaultBackfillPlan,
    repo: &R,
    scope: &Scope,
    report: &mut BackfillReport,
) -> Result<()> {
    let mut snapshot = repo.load_snapshot()?;
    let now = OffsetDateTime::now_utc();
    let mut changed = false;
    let mut page_events = Vec::new();
    let mut existing_page_written = existing_page_written_ids(repo)?;

    for source in &plan.sources {
        let desired_body = source.markdown.body.trim().to_string();
        match snapshot
            .sources
            .iter_mut()
            .find(|existing| existing.id == source.source_id)
        {
            Some(existing) => {
                let changed_record = existing.uri != source.uri
                    || existing.body != desired_body
                    || existing.scope != *scope
                    || existing.tags != source.tags;
                if changed_record {
                    existing.uri = source.uri.clone();
                    existing.body = desired_body;
                    existing.scope = scope.clone();
                    existing.tags = source.tags.clone();
                    report.sources_updated += 1;
                    changed = true;
                    mark_record_updated(report, &source.path);
                }
            }
            None => {
                snapshot.sources.push(RawArtifact {
                    id: source.source_id,
                    uri: source.uri.clone(),
                    body: desired_body,
                    scope: scope.clone(),
                    tags: source.tags.clone(),
                    ingested_at: now,
                });
                report.sources_imported += 1;
                changed = true;
                mark_record_imported(report, &source.path);
            }
        }
    }

    for page in &plan.pages {
        let mut desired = WikiPage {
            id: page.page_id,
            title: page.title.clone(),
            markdown: page.markdown.body.trim().to_string(),
            scope: scope.clone(),
            updated_at: now,
            outbound_page_titles: Vec::new(),
            entry_type: Some(page.entry_type.clone()),
            status: page.status,
            created_at: Some(now),
            status_entered_at: Some(now),
        };
        desired.refresh_outbound_links();

        let imported = match snapshot
            .pages
            .iter_mut()
            .find(|existing| existing.id == page.page_id)
        {
            Some(existing) => {
                let changed_record = existing.title != desired.title
                    || existing.markdown != desired.markdown
                    || existing.scope != desired.scope
                    || existing.outbound_page_titles != desired.outbound_page_titles
                    || existing.entry_type != desired.entry_type
                    || existing.status != desired.status;
                if changed_record {
                    let status_changed = existing.status != desired.status;
                    existing.title = desired.title;
                    existing.markdown = desired.markdown;
                    existing.scope = desired.scope;
                    existing.updated_at = now;
                    existing.outbound_page_titles = desired.outbound_page_titles;
                    existing.entry_type = desired.entry_type;
                    existing.status = desired.status;
                    if status_changed {
                        existing.status_entered_at = Some(now);
                    }
                    if existing.created_at.is_none() {
                        existing.created_at = Some(now);
                    }
                    report.pages_updated += 1;
                    changed = true;
                    mark_record_updated(report, &page.path);
                }
                false
            }
            None => {
                snapshot.pages.push(WikiPage {
                    id: page.page_id,
                    title: page.title.clone(),
                    markdown: page.markdown.body.trim().to_string(),
                    scope: scope.clone(),
                    updated_at: now,
                    outbound_page_titles: Vec::new(),
                    entry_type: Some(page.entry_type.clone()),
                    status: page.status,
                    created_at: Some(now),
                    status_entered_at: Some(now),
                });
                if let Some(inserted) = snapshot.pages.iter_mut().find(|p| p.id == page.page_id) {
                    inserted.refresh_outbound_links();
                }
                report.pages_imported += 1;
                changed = true;
                mark_record_imported(report, &page.path);
                true
            }
        };
        if (imported || !existing_page_written.contains(&page.page_id))
            && existing_page_written.insert(page.page_id)
        {
            page_events.push(WikiEvent::PageWritten {
                page_id: page.page_id,
                at: now,
            });
        }
    }

    if changed {
        repo.save_snapshot(&snapshot)?;
    }
    for event in page_events {
        repo.append_outbox(&event)?;
        report.page_written_events += 1;
    }
    Ok(())
}

fn existing_page_written_ids<R: WikiRepository>(repo: &R) -> Result<HashSet<PageId>> {
    let mut ids = HashSet::new();
    for line in repo.export_outbox_ndjson()?.lines() {
        if let Ok(WikiEvent::PageWritten { page_id, .. }) = serde_json::from_str(line) {
            ids.insert(page_id);
        }
    }
    Ok(ids)
}

fn mark_record_imported(report: &mut BackfillReport, path: &Path) {
    if let Some(record) = report.records.iter_mut().find(|record| record.path == path) {
        record.imported = true;
    }
}

fn mark_record_updated(report: &mut BackfillReport, path: &Path) {
    if let Some(record) = report.records.iter_mut().find(|record| record.path == path) {
        record.updated = true;
    }
}

fn parse_markdown_file(path: &Path) -> io::Result<MarkdownDoc> {
    let content = fs::read_to_string(path)?;
    let (frontmatter, body) = split_frontmatter(&content).unwrap_or(("", content.as_str()));
    let values = parse_frontmatter_values(frontmatter);
    Ok(MarkdownDoc {
        frontmatter: frontmatter.to_string(),
        body: body.to_string(),
        values,
    })
}

fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let idx = rest.find("\n---")?;
    let fm = &rest[..idx];
    let after_marker = &rest[idx + "\n---".len()..];
    let body = after_marker
        .strip_prefix("\r\n")
        .or_else(|| after_marker.strip_prefix('\n'))
        .unwrap_or(after_marker);
    Some((fm, body))
}

fn parse_frontmatter_values(frontmatter: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
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
        values.insert(key.to_string(), unquote_yaml_scalar(value.trim()));
    }
    values
}

fn write_doc_with_inserted_id(
    path: &Path,
    markdown: &MarkdownDoc,
    key: &str,
    id: Uuid,
) -> Result<()> {
    let mut frontmatter = String::new();
    frontmatter.push_str(&format!("{key}: \"{id}\"\n"));
    frontmatter.push_str(markdown.frontmatter.trim_end());
    frontmatter.push('\n');

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&frontmatter);
    content.push_str("---\n");
    if !markdown.body.starts_with('\n') {
        content.push('\n');
    }
    content.push_str(&markdown.body);
    fs::write(path, content)?;
    Ok(())
}

fn write_report_files(report_dir: &Path, report: &BackfillReport) -> Result<()> {
    fs::create_dir_all(report_dir)?;
    let json_path = report_dir.join("vault-backfill-report.json");
    let md_path = report_dir.join("vault-backfill-report.md");
    fs::write(json_path, serde_json::to_string_pretty(report)?)?;
    fs::write(md_path, render_markdown_report(report))?;
    Ok(())
}

fn render_markdown_report(report: &BackfillReport) -> String {
    format!(
        "# Vault Backfill Report\n\n- mode: {}\n- sources_seen: {}\n- pages_seen: {}\n- source_id_writes_planned: {}\n- page_id_writes_planned: {}\n- source_id_writes_applied: {}\n- page_id_writes_applied: {}\n- sources_imported: {}\n- pages_imported: {}\n- sources_updated: {}\n- pages_updated: {}\n- page_written_events: {}\n- skipped: {}\n",
        report.mode,
        report.sources_seen,
        report.pages_seen,
        report.source_id_writes_planned,
        report.page_id_writes_planned,
        report.source_id_writes_applied,
        report.page_id_writes_applied,
        report.sources_imported,
        report.pages_imported,
        report.sources_updated,
        report.pages_updated,
        report.page_written_events,
        report.skipped.len()
    )
}

fn has_non_empty(values: &HashMap<String, String>, key: &str) -> bool {
    values
        .get(key)
        .is_some_and(|value| !value.trim().is_empty())
}

fn parse_tags(raw: Option<&String>) -> Vec<String> {
    raw.map(|value| {
        value
            .trim_matches(['[', ']'])
            .split([',', '，'])
            .map(|tag| tag.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|tag| !tag.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn unquote_yaml_scalar(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_string()
}

fn relative_slash_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn file_stem_title(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string())
}

fn entry_type_from_path(rel_path: &str) -> EntryType {
    let lower = rel_path.to_ascii_lowercase();
    if lower.contains("/entity/") || lower.starts_with("pages/entity/") {
        EntryType::Entity
    } else if lower.contains("/summary/") || lower.starts_with("pages/summary/") {
        EntryType::Summary
    } else if lower.contains("/synthesis/") || lower.starts_with("pages/synthesis/") {
        EntryType::Synthesis
    } else if lower.contains("/qa/") || lower.starts_with("pages/qa/") {
        EntryType::Qa
    } else if lower.contains("/lint-report/") || lower.contains("/lint_report/") {
        EntryType::LintReport
    } else if lower.ends_with("/index.md") || lower == "index.md" {
        EntryType::Index
    } else {
        EntryType::Concept
    }
}

fn entry_type_name(entry_type: &EntryType) -> &'static str {
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

fn stable_v5_uuid(kind: &str, rel_path: &str, notion_uuid: &str) -> Uuid {
    let name = format!(
        "wiki-mempalace:vault-backfill:{kind}:{}:{}",
        rel_path.trim_start_matches('/'),
        notion_uuid.trim()
    );
    let namespace = Uuid::NAMESPACE_URL.as_bytes();
    let mut bytes = Vec::with_capacity(namespace.len() + name.len());
    bytes.extend_from_slice(namespace);
    bytes.extend_from_slice(name.as_bytes());
    let mut digest = sha1_digest(&bytes);
    digest[6] = (digest[6] & 0x0f) | 0x50;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    Uuid::from_bytes(digest[..16].try_into().expect("sha1 digest has 20 bytes"))
}

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xefcd_ab89;
    let mut h2: u32 = 0x98ba_dcfe;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xc3d2_e1f0;

    let bit_len = (input.len() as u64) * 8;
    let mut msg = input.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (idx, word) in w.iter_mut().take(16).enumerate() {
            let start = idx * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for idx in 16..80 {
            w[idx] = (w[idx - 3] ^ w[idx - 8] ^ w[idx - 14] ^ w[idx - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (idx, word) in w.iter().enumerate() {
            let (f, k) = match idx {
                0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_known_vector() {
        let digest = sha1_digest(b"abc");
        assert_eq!(
            hex_bytes(&digest),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn stable_id_is_v5_and_repeatable() {
        let a = stable_v5_uuid("source", "sources/a.md", "n1");
        let b = stable_v5_uuid("source", "sources/a.md", "n1");
        assert_eq!(a, b);
        assert_eq!(a.get_version_num(), 5);
    }

    fn hex_bytes(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
