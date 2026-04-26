use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use walkdir::WalkDir;

const SAMPLE_LIMIT: usize = 10;

#[derive(Debug, Clone, Serialize)]
pub struct VaultAuditReport {
    pub vault_path: String,
    pub generated_at: String,
    pub totals: VaultAuditTotals,
    pub frontmatter: FrontmatterStats,
    pub sources: SourceStats,
    pub pages: PageStats,
    pub identities: IdentityStats,
    pub orphan_candidates: OrphanCandidateStats,
    pub readiness: BackfillReadinessStats,
    pub path_lists: VaultAuditPathLists,
    pub old_audit_files: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VaultAuditTotals {
    pub total_files: usize,
    pub markdown_files: usize,
    pub source_files: usize,
    pub page_files: usize,
    pub root_files: usize,
    pub report_files: usize,
    pub wiki_artifact_files: usize,
    pub other_files: usize,
    pub old_orphan_audit_files: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct FrontmatterStats {
    pub markdown_files: usize,
    pub with_frontmatter: usize,
    pub missing_frontmatter: usize,
    pub unterminated_frontmatter: usize,
    pub invalid_utf8: usize,
    pub unsupported_lines: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceStats {
    pub total: usize,
    pub by_directory: BTreeMap<String, usize>,
    pub by_origin: BTreeMap<String, usize>,
    pub compiled_to_wiki: BoolValueStats,
    pub with_frontmatter: usize,
    pub missing_frontmatter: usize,
    pub invalid_frontmatter: usize,
    pub invalid_utf8: usize,
    pub with_source_id: usize,
    pub missing_source_id: usize,
    pub with_notion_uuid: usize,
    pub missing_notion_uuid: usize,
    pub missing_required_fields: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PageStats {
    pub total: usize,
    pub by_directory: BTreeMap<String, usize>,
    pub by_entry_type: BTreeMap<String, usize>,
    pub by_status: BTreeMap<String, usize>,
    pub with_frontmatter: usize,
    pub missing_frontmatter: usize,
    pub invalid_frontmatter: usize,
    pub invalid_utf8: usize,
    pub with_page_id: usize,
    pub missing_page_id: usize,
    pub with_notion_uuid: usize,
    pub missing_notion_uuid: usize,
    pub missing_entry_type: usize,
    pub unsupported_entry_type: usize,
    pub missing_status: usize,
    pub unsupported_directory: usize,
    pub missing_required_fields: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BoolValueStats {
    pub true_count: usize,
    pub false_count: usize,
    pub missing: usize,
    pub other: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct IdentityStats {
    pub source_id_values: usize,
    pub page_id_values: usize,
    pub notion_uuid_values: usize,
    pub duplicate_source_ids: Vec<DuplicateIdentityCandidate>,
    pub duplicate_page_ids: Vec<DuplicateIdentityCandidate>,
    pub duplicate_notion_uuids: Vec<DuplicateIdentityCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateIdentityCandidate {
    pub field: String,
    pub value: String,
    pub count: usize,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct OrphanCandidateStats {
    pub total_files: usize,
    pub by_category: BTreeMap<String, usize>,
    pub samples_by_category: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VaultAuditPathLists {
    pub pages_missing_status: Vec<String>,
    pub sources_missing_compiled_to_wiki: Vec<String>,
    pub unsupported_frontmatter: Vec<String>,
    pub orphan_candidates: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BackfillReadinessStats {
    pub ready_sources: usize,
    pub ready_pages: usize,
    pub missing_stable_id: usize,
    pub duplicate_identity_candidate_files: usize,
    pub unsupported_frontmatter: usize,
    pub orphan_candidate_files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultAuditReportFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ScannedMarkdown {
    rel_path: String,
    kind: MarkdownKind,
    frontmatter: FrontmatterResult,
    source_id: Option<String>,
    page_id: Option<String>,
    notion_uuid: Option<String>,
    orphan_categories: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkdownKind {
    Source,
    Page,
    Report,
    Other,
}

#[derive(Debug, Clone)]
enum FrontmatterResult {
    Parsed(ParsedFrontmatter),
    Missing,
    Unterminated,
    InvalidUtf8,
}

#[derive(Debug, Clone, Default)]
struct ParsedFrontmatter {
    fields: BTreeMap<String, FrontmatterValue>,
    unsupported_lines: usize,
}

#[derive(Debug, Clone)]
enum FrontmatterValue {
    Scalar(String),
    List(Vec<String>),
}

pub fn scan_vault(vault_path: impl AsRef<Path>) -> io::Result<VaultAuditReport> {
    let vault_path = vault_path.as_ref();
    let metadata = fs::metadata(vault_path)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("vault path is not a directory: {}", vault_path.display()),
        ));
    }

    let generated_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let mut report = VaultAuditReport {
        vault_path: vault_path.display().to_string(),
        generated_at,
        totals: VaultAuditTotals::default(),
        frontmatter: FrontmatterStats::default(),
        sources: SourceStats::default(),
        pages: PageStats::default(),
        identities: IdentityStats::default(),
        orphan_candidates: OrphanCandidateStats::default(),
        readiness: BackfillReadinessStats::default(),
        path_lists: VaultAuditPathLists::default(),
        old_audit_files: Vec::new(),
        warnings: Vec::new(),
    };

    let mut files = collect_content_files(vault_path, &mut report.warnings)?;
    files.sort();

    let mut scanned = Vec::new();
    let mut source_ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut page_ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut notion_uuids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut orphan_file_paths = BTreeSet::new();

    for path in files {
        report.totals.total_files += 1;
        let rel_path = relative_slash_path(vault_path, &path);
        let rel_parts = rel_parts(&rel_path);
        let is_markdown = has_extension(&path, "md");

        if rel_parts.len() == 1 {
            report.totals.root_files += 1;
        }
        if rel_path == ".wiki/orphan-audit.json" {
            report.totals.old_orphan_audit_files += 1;
            push_sample(&mut report.old_audit_files, rel_path.clone());
        }
        if first_part(&rel_parts) == Some(".wiki") {
            report.totals.wiki_artifact_files += 1;
        }
        if first_part(&rel_parts) == Some("reports") {
            report.totals.report_files += 1;
        }
        if !is_markdown {
            report.totals.other_files += 1;
            continue;
        }

        report.totals.markdown_files += 1;
        report.frontmatter.markdown_files += 1;

        let kind = markdown_kind(&rel_parts);
        match kind {
            MarkdownKind::Source => report.totals.source_files += 1,
            MarkdownKind::Page => report.totals.page_files += 1,
            MarkdownKind::Report | MarkdownKind::Other => {}
        }

        let frontmatter = match fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => parse_frontmatter(&text),
                Err(err) => {
                    report.warnings.push(format!(
                        "invalid UTF-8 markdown counted as unreadable: {} ({err})",
                        rel_path
                    ));
                    FrontmatterResult::InvalidUtf8
                }
            },
            Err(err) => {
                report
                    .warnings
                    .push(format!("failed to read markdown: {} ({err})", rel_path));
                FrontmatterResult::Missing
            }
        };
        update_frontmatter_stats(&frontmatter, &mut report.frontmatter);

        let source_id = frontmatter.scalar("source_id");
        let page_id = frontmatter.scalar("page_id");
        let notion_uuid = frontmatter.scalar("notion_uuid");
        if let Some(value) = &source_id {
            source_ids
                .entry(value.clone())
                .or_default()
                .push(rel_path.clone());
        }
        if let Some(value) = &page_id {
            page_ids
                .entry(value.clone())
                .or_default()
                .push(rel_path.clone());
        }
        if let Some(value) = &notion_uuid {
            notion_uuids
                .entry(value.clone())
                .or_default()
                .push(rel_path.clone());
        }

        let mut orphan_categories = Vec::new();
        match kind {
            MarkdownKind::Source => update_source_stats(
                &rel_path,
                &rel_parts,
                &frontmatter,
                source_id.as_deref(),
                notion_uuid.as_deref(),
                &mut report.sources,
                &mut orphan_categories,
            ),
            MarkdownKind::Page => update_page_stats(
                &rel_path,
                &rel_parts,
                &frontmatter,
                page_id.as_deref(),
                notion_uuid.as_deref(),
                &mut report.pages,
                &mut orphan_categories,
            ),
            MarkdownKind::Report => {}
            MarkdownKind::Other => {
                if !is_allowed_root_projection(&rel_path) {
                    orphan_categories.push("unclassified_markdown".to_string());
                }
            }
        }
        match kind {
            MarkdownKind::Source => {
                if frontmatter.scalar("compiled_to_wiki").is_none() {
                    report
                        .path_lists
                        .sources_missing_compiled_to_wiki
                        .push(rel_path.clone());
                }
            }
            MarkdownKind::Page => {
                if frontmatter.scalar("status").is_none() {
                    report
                        .path_lists
                        .pages_missing_status
                        .push(rel_path.clone());
                }
            }
            MarkdownKind::Report | MarkdownKind::Other => {}
        }
        if has_unsupported_frontmatter(&frontmatter) {
            report
                .path_lists
                .unsupported_frontmatter
                .push(rel_path.clone());
        }
        for category in &orphan_categories {
            add_orphan_candidate(&mut report.orphan_candidates, category, &rel_path);
        }
        if !orphan_categories.is_empty() {
            orphan_file_paths.insert(rel_path.clone());
            report.path_lists.orphan_candidates.push(rel_path.clone());
        }

        scanned.push(ScannedMarkdown {
            rel_path,
            kind,
            frontmatter,
            source_id,
            page_id,
            notion_uuid,
            orphan_categories,
        });
    }

    report.identities.source_id_values = source_ids.len();
    report.identities.page_id_values = page_ids.len();
    report.identities.notion_uuid_values = notion_uuids.len();
    let duplicate_files = duplicate_file_set_from_values([&source_ids, &page_ids, &notion_uuids]);
    report.identities.duplicate_source_ids = duplicate_candidates("source_id", &source_ids);
    report.identities.duplicate_page_ids = duplicate_candidates("page_id", &page_ids);
    report.identities.duplicate_notion_uuids = duplicate_candidates("notion_uuid", &notion_uuids);

    update_readiness(&mut report, &scanned, &duplicate_files);
    report.orphan_candidates.total_files = orphan_file_paths.len();
    Ok(report)
}

pub fn write_json_and_markdown(
    report: &VaultAuditReport,
    report_dir: impl AsRef<Path>,
) -> Result<VaultAuditReportFiles, Box<dyn std::error::Error + Send + Sync>> {
    let report_dir = report_dir.as_ref();
    validate_report_dir(report, report_dir)?;
    fs::create_dir_all(report_dir)?;
    let stem = format!("vault-audit-{}", filename_timestamp(&report.generated_at));
    let json_name = format!("{stem}.json");
    let markdown_name = format!("{stem}.md");
    let json_path = report_dir.join(&json_name);
    let markdown_path = report_dir.join(&markdown_name);
    fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    fs::write(&markdown_path, render_markdown(report, &json_name))?;
    Ok(VaultAuditReportFiles {
        json_path,
        markdown_path,
    })
}

pub fn write_json_and_markdown_in_vault_reports(
    report: &VaultAuditReport,
) -> Result<VaultAuditReportFiles, Box<dyn std::error::Error + Send + Sync>> {
    write_json_and_markdown(report, Path::new(&report.vault_path).join("reports"))
}

pub fn render_markdown(report: &VaultAuditReport, sibling_json: &str) -> String {
    let mut out = format!(
        concat!(
            "# Vault Audit\n\n",
            "- vault_path: `{}`\n",
            "- generated_at: `{}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth. This Markdown is rendered from the same VaultAuditReport.\n\n",
            "## Totals\n\n",
            "- total_files: {}\n",
            "- markdown_files: {}\n",
            "- source_files: {}\n",
            "- page_files: {}\n",
            "- root_files: {}\n",
            "- report_files: {}\n",
            "- wiki_artifact_files: {}\n",
            "- old_orphan_audit_files: {}\n\n",
            "## Frontmatter\n\n",
            "- with_frontmatter: {}\n",
            "- missing_frontmatter: {}\n",
            "- unterminated_frontmatter: {}\n",
            "- invalid_utf8: {}\n",
            "- unsupported_lines: {}\n\n",
        ),
        report.vault_path,
        report.generated_at,
        sibling_json,
        sibling_json,
        report.totals.total_files,
        report.totals.markdown_files,
        report.totals.source_files,
        report.totals.page_files,
        report.totals.root_files,
        report.totals.report_files,
        report.totals.wiki_artifact_files,
        report.totals.old_orphan_audit_files,
        report.frontmatter.with_frontmatter,
        report.frontmatter.missing_frontmatter,
        report.frontmatter.unterminated_frontmatter,
        report.frontmatter.invalid_utf8,
        report.frontmatter.unsupported_lines
    );

    out.push_str("## Sources\n\n");
    out.push_str(&format!(
        concat!(
            "- total: {}\n",
            "- source_id: with={} missing={}\n",
            "- notion_uuid: with={} missing={}\n",
            "- compiled_to_wiki: true={} false={} missing={} other={}\n",
            "- invalid_utf8: {}\n\n",
        ),
        report.sources.total,
        report.sources.with_source_id,
        report.sources.missing_source_id,
        report.sources.with_notion_uuid,
        report.sources.missing_notion_uuid,
        report.sources.compiled_to_wiki.true_count,
        report.sources.compiled_to_wiki.false_count,
        report.sources.compiled_to_wiki.missing,
        report.sources.compiled_to_wiki.other,
        report.sources.invalid_utf8
    ));
    push_map_section(&mut out, "Source Origins", &report.sources.by_origin);

    out.push_str("## Pages\n\n");
    out.push_str(&format!(
        concat!(
            "- total: {}\n",
            "- page_id: with={} missing={}\n",
            "- notion_uuid: with={} missing={}\n",
            "- missing_entry_type: {}\n",
            "- unsupported_entry_type: {}\n",
            "- missing_status: {}\n",
            "- unsupported_directory: {}\n",
            "- invalid_utf8: {}\n\n",
        ),
        report.pages.total,
        report.pages.with_page_id,
        report.pages.missing_page_id,
        report.pages.with_notion_uuid,
        report.pages.missing_notion_uuid,
        report.pages.missing_entry_type,
        report.pages.unsupported_entry_type,
        report.pages.missing_status,
        report.pages.unsupported_directory,
        report.pages.invalid_utf8
    ));
    push_map_section(&mut out, "Page Entry Types", &report.pages.by_entry_type);
    push_map_section(&mut out, "Page Status", &report.pages.by_status);

    out.push_str("## Backfill Readiness\n\n");
    out.push_str(&format!(
        concat!(
            "- ready_sources: {}\n",
            "- ready_pages: {}\n",
            "- missing_stable_id: {}\n",
            "- duplicate_identity_candidate_files: {}\n",
            "- unsupported_frontmatter: {}\n",
            "- orphan_candidate_files: {}\n\n",
        ),
        report.readiness.ready_sources,
        report.readiness.ready_pages,
        report.readiness.missing_stable_id,
        report.readiness.duplicate_identity_candidate_files,
        report.readiness.unsupported_frontmatter,
        report.readiness.orphan_candidate_files
    ));

    push_map_section(
        &mut out,
        "Fresh Orphan Candidate Categories",
        &report.orphan_candidates.by_category,
    );
    if !report.old_audit_files.is_empty() {
        out.push_str("## Old Audit Files\n\n");
        for path in &report.old_audit_files {
            out.push_str(&format!("- `{path}`\n"));
        }
        out.push('\n');
    }
    if !report.warnings.is_empty() {
        out.push_str("## Warnings\n\n");
        for warning in &report.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }
    out
}

fn collect_content_files(
    vault_path: &Path,
    warnings: &mut Vec<String>,
) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for content_dir in ["pages", "sources"] {
        let root = vault_path.join(content_dir);
        match fs::metadata(&root) {
            Ok(metadata) if metadata.is_dir() => {
                for entry in WalkDir::new(&root).follow_links(false) {
                    match entry {
                        Ok(entry) => {
                            if entry.file_type().is_file() {
                                files.push(entry.path().to_path_buf());
                            }
                        }
                        Err(err) => warnings.push(format!("failed to scan path: {err}")),
                    }
                }
            }
            Ok(_) => warnings.push(format!(
                "content path is not a directory and was skipped: {}",
                root.display()
            )),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => warnings.push(format!(
                "failed to scan content path: {} ({err})",
                root.display()
            )),
        }
    }
    Ok(files)
}

fn filename_timestamp(timestamp: &str) -> String {
    let cleaned: String = timestamp
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | 'T' | 'Z') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    cleaned.trim_matches('-').to_string()
}

fn validate_report_dir(report: &VaultAuditReport, report_dir: &Path) -> io::Result<()> {
    let vault_reports_dir = normalize_path(Path::new(&report.vault_path).join("reports"))?;
    let report_dir = normalize_path(report_dir)?;
    if report_dir.starts_with(&vault_reports_dir) {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "vault audit reports must be written under {}; got {}",
            vault_reports_dir.display(),
            report_dir.display()
        ),
    ))
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

fn parse_frontmatter(content: &str) -> FrontmatterResult {
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return FrontmatterResult::Missing;
    }

    let mut parsed = ParsedFrontmatter::default();
    let mut current_list_key: Option<String> = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            return FrontmatterResult::Parsed(parsed);
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some(key) = &current_list_key {
                if let Some(FrontmatterValue::List(values)) = parsed.fields.get_mut(key) {
                    values.push(clean_scalar(item));
                    continue;
                }
            }
            parsed.unsupported_lines += 1;
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            parsed.unsupported_lines += 1;
            current_list_key = None;
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim();
        if key.is_empty() {
            parsed.unsupported_lines += 1;
            current_list_key = None;
            continue;
        }
        if value.is_empty() {
            parsed
                .fields
                .insert(key.clone(), FrontmatterValue::List(Vec::new()));
            current_list_key = Some(key);
        } else if let Some(values) = parse_inline_list(value) {
            parsed.fields.insert(key, FrontmatterValue::List(values));
            current_list_key = None;
        } else {
            parsed
                .fields
                .insert(key, FrontmatterValue::Scalar(clean_scalar(value)));
            current_list_key = None;
        }
    }

    FrontmatterResult::Unterminated
}

fn update_frontmatter_stats(frontmatter: &FrontmatterResult, stats: &mut FrontmatterStats) {
    match frontmatter {
        FrontmatterResult::Parsed(parsed) => {
            stats.with_frontmatter += 1;
            stats.unsupported_lines += parsed.unsupported_lines;
        }
        FrontmatterResult::Missing => stats.missing_frontmatter += 1,
        FrontmatterResult::Unterminated => stats.unterminated_frontmatter += 1,
        FrontmatterResult::InvalidUtf8 => stats.invalid_utf8 += 1,
    }
}

fn has_unsupported_frontmatter(frontmatter: &FrontmatterResult) -> bool {
    matches!(
        frontmatter,
        FrontmatterResult::Missing
            | FrontmatterResult::Unterminated
            | FrontmatterResult::InvalidUtf8
    ) || matches!(
        frontmatter,
        FrontmatterResult::Parsed(parsed) if parsed.unsupported_lines > 0
    )
}

fn update_source_stats(
    rel_path: &str,
    rel_parts: &[&str],
    frontmatter: &FrontmatterResult,
    source_id: Option<&str>,
    notion_uuid: Option<&str>,
    stats: &mut SourceStats,
    orphan_categories: &mut Vec<String>,
) {
    stats.total += 1;
    increment(&mut stats.by_directory, parent_dir(rel_path));
    if rel_parts.len() == 2 {
        orphan_categories.push("source_root_file".to_string());
    }

    let origin = frontmatter
        .scalar("origin")
        .or_else(|| {
            if rel_parts.len() > 2 {
                rel_parts.get(1).map(|s| (*s).to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "missing".to_string());
    increment(&mut stats.by_origin, origin);

    match frontmatter {
        FrontmatterResult::Parsed(parsed) => {
            stats.with_frontmatter += 1;
            if parsed.unsupported_lines > 0 {
                stats.invalid_frontmatter += 1;
                orphan_categories.push("unsupported_frontmatter".to_string());
            }
            update_bool_stats(
                &mut stats.compiled_to_wiki,
                frontmatter.scalar("compiled_to_wiki"),
            );
            for field in ["title", "kind", "origin", "compiled_to_wiki", "created_at"] {
                if !parsed.has_field(field) {
                    increment(&mut stats.missing_required_fields, field.to_string());
                }
            }
        }
        FrontmatterResult::Missing => {
            stats.missing_frontmatter += 1;
            stats.compiled_to_wiki.missing += 1;
            orphan_categories.push("missing_frontmatter".to_string());
        }
        FrontmatterResult::Unterminated => {
            stats.invalid_frontmatter += 1;
            stats.compiled_to_wiki.missing += 1;
            orphan_categories.push("unterminated_frontmatter".to_string());
        }
        FrontmatterResult::InvalidUtf8 => {
            stats.invalid_frontmatter += 1;
            stats.invalid_utf8 += 1;
            stats.compiled_to_wiki.missing += 1;
            orphan_categories.push("invalid_utf8".to_string());
        }
    }

    if source_id.is_some() {
        stats.with_source_id += 1;
    } else {
        stats.missing_source_id += 1;
    }
    if notion_uuid.is_some() {
        stats.with_notion_uuid += 1;
    } else {
        stats.missing_notion_uuid += 1;
    }
}

fn update_page_stats(
    rel_path: &str,
    rel_parts: &[&str],
    frontmatter: &FrontmatterResult,
    page_id: Option<&str>,
    notion_uuid: Option<&str>,
    stats: &mut PageStats,
    orphan_categories: &mut Vec<String>,
) {
    stats.total += 1;
    let directory = parent_dir(rel_path);
    increment(&mut stats.by_directory, directory.clone());
    if !is_supported_page_directory(&directory) {
        stats.unsupported_directory += 1;
        orphan_categories.push("unsupported_page_directory".to_string());
    }
    if rel_parts.len() == 2 {
        orphan_categories.push("page_root_file".to_string());
    }

    match frontmatter {
        FrontmatterResult::Parsed(parsed) => {
            stats.with_frontmatter += 1;
            if parsed.unsupported_lines > 0 {
                stats.invalid_frontmatter += 1;
                orphan_categories.push("unsupported_frontmatter".to_string());
            }
            if let Some(entry_type) = frontmatter.scalar("entry_type") {
                let normalized = normalize_entry_type(&entry_type);
                increment(&mut stats.by_entry_type, normalized.clone());
                if !is_supported_entry_type(&normalized) {
                    stats.unsupported_entry_type += 1;
                    orphan_categories.push("unsupported_entry_type".to_string());
                }
            } else {
                stats.missing_entry_type += 1;
                orphan_categories.push("missing_entry_type".to_string());
            }
            if let Some(status) = frontmatter.scalar("status") {
                increment(&mut stats.by_status, status.to_ascii_lowercase());
            } else {
                stats.missing_status += 1;
            }
            for field in ["title", "entry_type", "status"] {
                if !parsed.has_field(field) {
                    increment(&mut stats.missing_required_fields, field.to_string());
                }
            }
        }
        FrontmatterResult::Missing => {
            stats.missing_frontmatter += 1;
            stats.missing_entry_type += 1;
            stats.missing_status += 1;
            orphan_categories.push("missing_frontmatter".to_string());
        }
        FrontmatterResult::Unterminated => {
            stats.invalid_frontmatter += 1;
            stats.missing_entry_type += 1;
            stats.missing_status += 1;
            orphan_categories.push("unterminated_frontmatter".to_string());
        }
        FrontmatterResult::InvalidUtf8 => {
            stats.invalid_frontmatter += 1;
            stats.invalid_utf8 += 1;
            stats.missing_entry_type += 1;
            stats.missing_status += 1;
            orphan_categories.push("invalid_utf8".to_string());
        }
    }

    if page_id.is_some() {
        stats.with_page_id += 1;
    } else {
        stats.missing_page_id += 1;
    }
    if notion_uuid.is_some() {
        stats.with_notion_uuid += 1;
    } else {
        stats.missing_notion_uuid += 1;
    }
}

fn update_readiness(
    report: &mut VaultAuditReport,
    scanned: &[ScannedMarkdown],
    duplicate_files: &BTreeSet<String>,
) {
    report.readiness.duplicate_identity_candidate_files = duplicate_files.len();

    let mut missing_stable = BTreeSet::new();
    let mut unsupported_frontmatter = BTreeSet::new();
    let mut orphan_files = BTreeSet::new();
    for file in scanned {
        if !file.orphan_categories.is_empty() {
            orphan_files.insert(file.rel_path.clone());
        }
        if matches!(
            file.frontmatter,
            FrontmatterResult::Missing
                | FrontmatterResult::Unterminated
                | FrontmatterResult::InvalidUtf8
        ) || matches!(
            &file.frontmatter,
            FrontmatterResult::Parsed(parsed) if parsed.unsupported_lines > 0
        ) {
            unsupported_frontmatter.insert(file.rel_path.clone());
        }
        match file.kind {
            MarkdownKind::Source => {
                if file.source_id.is_none() && file.notion_uuid.is_none() {
                    missing_stable.insert(file.rel_path.clone());
                } else if !unsupported_frontmatter.contains(&file.rel_path)
                    && !duplicate_files.contains(&file.rel_path)
                    && file.orphan_categories.is_empty()
                {
                    report.readiness.ready_sources += 1;
                }
            }
            MarkdownKind::Page => {
                if file.page_id.is_none() && file.notion_uuid.is_none() {
                    missing_stable.insert(file.rel_path.clone());
                } else if !unsupported_frontmatter.contains(&file.rel_path)
                    && !duplicate_files.contains(&file.rel_path)
                    && file.orphan_categories.is_empty()
                {
                    report.readiness.ready_pages += 1;
                }
            }
            MarkdownKind::Report | MarkdownKind::Other => {}
        }
    }
    report.readiness.missing_stable_id = missing_stable.len();
    report.readiness.unsupported_frontmatter = unsupported_frontmatter.len();
    report.readiness.orphan_candidate_files = orphan_files.len();
}

fn duplicate_candidates(
    field: &str,
    values: &BTreeMap<String, Vec<String>>,
) -> Vec<DuplicateIdentityCandidate> {
    values
        .iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(value, paths)| DuplicateIdentityCandidate {
            field: field.to_string(),
            value: value.clone(),
            count: paths.len(),
            paths: paths.iter().take(SAMPLE_LIMIT).cloned().collect(),
        })
        .collect()
}

fn duplicate_file_set_from_values<'a>(
    values: impl IntoIterator<Item = &'a BTreeMap<String, Vec<String>>>,
) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for value_map in values {
        for paths in value_map.values().filter(|paths| paths.len() > 1) {
            for path in paths {
                files.insert(path.clone());
            }
        }
    }
    files
}

fn add_orphan_candidate(stats: &mut OrphanCandidateStats, category: &str, path: &str) {
    increment(&mut stats.by_category, category.to_string());
    push_sample(
        stats
            .samples_by_category
            .entry(category.to_string())
            .or_default(),
        path.to_string(),
    );
}

fn update_bool_stats(stats: &mut BoolValueStats, value: Option<String>) {
    match value.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("true") => stats.true_count += 1,
        Some("false") => stats.false_count += 1,
        Some(_) => stats.other += 1,
        None => stats.missing += 1,
    }
}

fn markdown_kind(rel_parts: &[&str]) -> MarkdownKind {
    match first_part(rel_parts) {
        Some("sources") => MarkdownKind::Source,
        Some("pages") => MarkdownKind::Page,
        Some("reports") => MarkdownKind::Report,
        _ => MarkdownKind::Other,
    }
}

fn is_allowed_root_projection(rel_path: &str) -> bool {
    matches!(rel_path, "index.md" | "log.md")
}

fn is_supported_page_directory(directory: &str) -> bool {
    matches!(
        directory,
        "pages/summary"
            | "pages/concept"
            | "pages/entity"
            | "pages/synthesis"
            | "pages/qa"
            | "pages/index"
            | "pages/lint-report"
            | "pages/_unspecified"
    )
}

fn is_supported_entry_type(entry_type: &str) -> bool {
    matches!(
        entry_type,
        "summary" | "concept" | "entity" | "synthesis" | "qa" | "index" | "lint_report"
    )
}

fn normalize_entry_type(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

fn first_part<'a>(rel_parts: &'a [&str]) -> Option<&'a str> {
    rel_parts.first().copied()
}

fn rel_parts(rel_path: &str) -> Vec<&str> {
    rel_path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn parent_dir(rel_path: &str) -> String {
    Path::new(rel_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(path_to_slash_string)
        .unwrap_or_else(|| ".".to_string())
}

fn relative_slash_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(path_to_slash_string)
        .unwrap_or_else(|_| path_to_slash_string(path))
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

fn parse_inline_list(value: &str) -> Option<Vec<String>> {
    let value = value.trim();
    let inner = value.strip_prefix('[')?.strip_suffix(']')?;
    Some(
        inner
            .split(',')
            .map(clean_scalar)
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

fn clean_scalar(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn increment(map: &mut BTreeMap<String, usize>, key: String) {
    *map.entry(key).or_insert(0) += 1;
}

fn push_sample(samples: &mut Vec<String>, value: String) {
    if samples.len() < SAMPLE_LIMIT && !samples.contains(&value) {
        samples.push(value);
    }
}

fn push_map_section(out: &mut String, title: &str, map: &BTreeMap<String, usize>) {
    out.push_str(&format!("## {title}\n\n"));
    if map.is_empty() {
        out.push_str("- none\n\n");
        return;
    }
    for (key, value) in map {
        out.push_str(&format!("- {}: {}\n", key, value));
    }
    out.push('\n');
}

impl FrontmatterResult {
    fn scalar(&self, key: &str) -> Option<String> {
        match self {
            FrontmatterResult::Parsed(parsed) => parsed.scalar(key),
            FrontmatterResult::Missing
            | FrontmatterResult::Unterminated
            | FrontmatterResult::InvalidUtf8 => None,
        }
    }
}

impl ParsedFrontmatter {
    fn has_field(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    fn scalar(&self, key: &str) -> Option<String> {
        match self.fields.get(key) {
            Some(FrontmatterValue::Scalar(value)) if !value.is_empty() => Some(value.clone()),
            Some(FrontmatterValue::List(values)) if !values.is_empty() => Some(values.join(",")),
            _ => None,
        }
    }
}
