use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Serialize)]
pub struct OrphanGovernanceReport {
    pub generated_at: String,
    pub audit_report_path: String,
    pub audit_generated_at: Option<String>,
    pub vault_path: Option<String>,
    pub counts: GovernanceCounts,
    pub lanes: Vec<GovernanceLane>,
    pub mutation_policy: MutationPolicy,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct GovernanceCounts {
    pub orphan_candidates: usize,
    pub unsupported_frontmatter: usize,
    pub pages_missing_status: usize,
    pub sources_missing_compiled_to_wiki: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct GovernanceLane {
    pub lane: &'static str,
    pub finding: &'static str,
    pub count: usize,
    pub samples: Vec<String>,
    pub reason: &'static str,
    pub next_step: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct MutationPolicy {
    pub version: &'static str,
    pub writes_reports_only: bool,
    pub vault_markdown_mutation: bool,
    pub db_writes: bool,
    pub outbox_emission: bool,
    pub palace_writes: bool,
    pub apply_mode: bool,
    pub summary: &'static str,
}

#[derive(Debug, Clone)]
pub struct OrphanGovernanceReportFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct AuditInput {
    vault_path: Option<String>,
    generated_at: Option<String>,
    orphan_candidates: Option<AuditOrphanCandidates>,
    readiness: Option<AuditReadiness>,
    pages: Option<AuditPages>,
    sources: Option<AuditSources>,
}

#[derive(Debug, Default, Deserialize)]
struct AuditOrphanCandidates {
    total_files: Option<usize>,
    samples_by_category: Option<BTreeMap<String, Vec<String>>>,
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

pub fn run_orphan_governance(
    audit_report: impl AsRef<Path>,
    report_dir: Option<PathBuf>,
    wiki_dir: Option<&Path>,
) -> Result<
    (OrphanGovernanceReport, OrphanGovernanceReportFiles),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let audit_report = audit_report.as_ref();
    let report = build_report_from_audit_path(audit_report)?;
    let report_dir = resolve_report_dir(audit_report, report_dir, wiki_dir)?;
    let files = write_report_files(&report, &report_dir)?;
    Ok((report, files))
}

pub fn build_report_from_audit_path(
    audit_report: impl AsRef<Path>,
) -> Result<OrphanGovernanceReport, Box<dyn std::error::Error + Send + Sync>> {
    let audit_report = audit_report.as_ref();
    let body = fs::read_to_string(audit_report)?;
    let audit: AuditInput = serde_json::from_str(&body)?;
    let report = build_report(audit, audit_report)?;
    Ok(report)
}

fn build_report(audit: AuditInput, audit_report: &Path) -> io::Result<OrphanGovernanceReport> {
    let audit_generated_at = require_field(audit.generated_at, "generated_at")?;
    let vault_path = require_field(audit.vault_path, "vault_path")?;
    let orphan_candidates = require_field(audit.orphan_candidates, "orphan_candidates")?;
    let readiness = require_field(audit.readiness, "readiness")?;
    let pages = require_field(audit.pages, "pages")?;
    let sources = require_field(audit.sources, "sources")?;
    let compiled_to_wiki = require_field(sources.compiled_to_wiki, "sources.compiled_to_wiki")?;
    let counts = GovernanceCounts {
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
    };
    let orphan_samples = orphan_candidates
        .samples_by_category
        .unwrap_or_default()
        .into_values()
        .flatten()
        .collect::<Vec<_>>();

    Ok(OrphanGovernanceReport {
        generated_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string()),
        audit_report_path: audit_report.display().to_string(),
        audit_generated_at: Some(audit_generated_at),
        vault_path: Some(vault_path),
        lanes: classify_lanes(&counts, orphan_samples),
        counts,
        mutation_policy: MutationPolicy {
            version: "v1",
            writes_reports_only: true,
            vault_markdown_mutation: false,
            db_writes: false,
            outbox_emission: false,
            palace_writes: false,
            apply_mode: false,
            summary: "Reports only; no vault Markdown mutation, no DB/outbox/palace writes, no apply mode.",
        },
    })
}

fn require_field<T>(value: Option<T>, field: &str) -> io::Result<T> {
    value.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("audit report missing required field: {field}"),
        )
    })
}

fn classify_lanes(counts: &GovernanceCounts, orphan_samples: Vec<String>) -> Vec<GovernanceLane> {
    vec![
        GovernanceLane {
            lane: "human_required",
            finding: "orphan_candidates",
            count: counts.orphan_candidates,
            samples: orphan_samples,
            reason: "Samples may include old .wiki, _archive, or legacy Markdown. Move/delete/link decisions need human context.",
            next_step: "Review exact paths with a human before any archive move, deletion, or relink.",
        },
        GovernanceLane {
            lane: "agent_review",
            finding: "unsupported_frontmatter",
            count: counts.unsupported_frontmatter,
            samples: Vec::new(),
            reason: "Current audit exposes the count as readiness data, not enough path-level evidence for mutation.",
            next_step: "Collect path-level evidence in a future report before proposing fixes.",
        },
        GovernanceLane {
            lane: "future_auto_fix",
            finding: "pages_missing_status",
            count: counts.pages_missing_status,
            samples: Vec::new(),
            reason: "A future dry-run can propose status: draft after exact page paths are listed.",
            next_step: "Keep report-only in v1; require user approval before any auto-fix/apply mode.",
        },
        GovernanceLane {
            lane: "agent_review",
            finding: "sources_missing_compiled_to_wiki",
            count: counts.sources_missing_compiled_to_wiki,
            samples: Vec::new(),
            reason: "Choosing true or false changes ingestion semantics and can hide work or trigger recompile.",
            next_step: "Inspect each source before proposing compiled_to_wiki values.",
        },
    ]
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

pub fn write_report_files(
    report: &OrphanGovernanceReport,
    report_dir: &Path,
) -> Result<OrphanGovernanceReportFiles, Box<dyn std::error::Error + Send + Sync>> {
    fs::create_dir_all(report_dir)?;
    let json_path = report_dir.join("orphan-governance-report.json");
    let markdown_path = report_dir.join("orphan-governance-report.md");
    fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    fs::write(
        &markdown_path,
        render_markdown(report, "orphan-governance-report.json"),
    )?;
    Ok(OrphanGovernanceReportFiles {
        json_path,
        markdown_path,
    })
}

pub fn render_markdown(report: &OrphanGovernanceReport, sibling_json: &str) -> String {
    let mut out = format!(
        concat!(
            "# Orphan Governance Report\n\n",
            "- generated_at: `{}`\n",
            "- audit_report_path: `{}`\n",
            "- audit_generated_at: `{}`\n",
            "- vault_path: `{}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth. This Markdown is rendered from the same OrphanGovernanceReport.\n\n",
            "## Counts\n\n",
            "- orphan_candidates: {}\n",
            "- unsupported_frontmatter: {}\n",
            "- pages_missing_status: {}\n",
            "- sources_missing_compiled_to_wiki: {}\n\n",
            "## Mutation Policy\n\n",
            "- version: {}\n",
            "- writes_reports_only: {}\n",
            "- vault_markdown_mutation: {}\n",
            "- db_writes: {}\n",
            "- outbox_emission: {}\n",
            "- palace_writes: {}\n",
            "- apply_mode: {}\n",
            "- summary: {}\n\n",
            "## Lanes\n\n",
        ),
        report.generated_at,
        report.audit_report_path,
        report.audit_generated_at.as_deref().unwrap_or("unknown"),
        report.vault_path.as_deref().unwrap_or("unknown"),
        sibling_json,
        sibling_json,
        report.counts.orphan_candidates,
        report.counts.unsupported_frontmatter,
        report.counts.pages_missing_status,
        report.counts.sources_missing_compiled_to_wiki,
        report.mutation_policy.version,
        report.mutation_policy.writes_reports_only,
        report.mutation_policy.vault_markdown_mutation,
        report.mutation_policy.db_writes,
        report.mutation_policy.outbox_emission,
        report.mutation_policy.palace_writes,
        report.mutation_policy.apply_mode,
        report.mutation_policy.summary,
    );

    for lane in &report.lanes {
        out.push_str(&format!(
            concat!(
                "### {}\n\n",
                "- lane: `{}`\n",
                "- count: {}\n",
                "- reason: {}\n",
                "- next_step: {}\n",
            ),
            lane.finding, lane.lane, lane.count, lane.reason, lane.next_step
        ));
        if !lane.samples.is_empty() {
            out.push_str("- samples:\n");
            for sample in &lane.samples {
                out.push_str(&format!("  - `{sample}`\n"));
            }
        }
        out.push('\n');
    }

    out
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
