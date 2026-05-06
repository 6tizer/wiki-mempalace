use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use wiki_core::{EntryStatus, EntryType, MemoryTier, Scope};

// --- Constants for default paths ---

pub(crate) const DEFAULT_DASHBOARD_OUTPUT: &str = "wiki/reports/dashboard.html";
pub(crate) const VAULT_DASHBOARD_OUTPUT: &str = "reports/dashboard.html";
pub(crate) const DEFAULT_SUGGEST_REPORT_DIR: &str = "wiki/reports/suggestions";
pub(crate) const VAULT_SUGGEST_REPORT_DIR: &str = "reports/suggestions";
pub(crate) const DEFAULT_GOVERNANCE_REPORT_DIR: &str = "wiki/reports/governance";
pub(crate) const VAULT_GOVERNANCE_REPORT_DIR: &str = "reports/governance";
pub(crate) const DEFAULT_FIXER_REPORT_DIR: &str = "wiki/reports/fixer";
pub(crate) const VAULT_FIXER_REPORT_DIR: &str = "reports/fixer";
pub(crate) const DEFAULT_SYNTHESIS_REPORT_DIR: &str = "wiki/reports/synthesis";
pub(crate) const VAULT_SYNTHESIS_REPORT_DIR: &str = "reports/synthesis";

// --- Environment helpers ---

pub(crate) fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// --- Name mappings ---

pub(crate) fn entry_status_name(status: EntryStatus) -> &'static str {
    match status {
        EntryStatus::Draft => "draft",
        EntryStatus::InReview => "in_review",
        EntryStatus::Approved => "approved",
        EntryStatus::NeedsUpdate => "needs_update",
    }
}

pub(crate) fn entry_type_name(entry_type: &EntryType) -> &'static str {
    match entry_type {
        EntryType::Concept => "concept",
        EntryType::Entity => "entity",
        EntryType::Summary => "summary",
        EntryType::Synthesis => "synthesis",
        EntryType::Qa => "qa",
        EntryType::LintReport => "lint_report",
        EntryType::Index => "index",
        EntryType::Skill => "skill",
    }
}

pub(crate) fn format_optional_i64(value: Option<i64>) -> String {
    value
        .map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string())
}

// --- Path resolution ---

pub(crate) fn resolve_wiki_relative_path(wiki_root: Option<&Path>, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else if let Some(root) = wiki_root {
        root.join(path)
    } else {
        path
    }
}

pub(crate) fn default_dashboard_output(wiki_root: Option<&Path>) -> PathBuf {
    if let Some(root) = wiki_root {
        root.join(VAULT_DASHBOARD_OUTPUT)
    } else {
        PathBuf::from(DEFAULT_DASHBOARD_OUTPUT)
    }
}

pub(crate) fn default_suggest_report_dir(wiki_root: Option<&Path>) -> PathBuf {
    if let Some(root) = wiki_root {
        root.join(VAULT_SUGGEST_REPORT_DIR)
    } else {
        PathBuf::from(DEFAULT_SUGGEST_REPORT_DIR)
    }
}

pub(crate) fn default_governance_report_dir(wiki_root: Option<&Path>) -> PathBuf {
    if let Some(root) = wiki_root {
        root.join(VAULT_GOVERNANCE_REPORT_DIR)
    } else {
        PathBuf::from(DEFAULT_GOVERNANCE_REPORT_DIR)
    }
}

pub(crate) fn default_fixer_report_dir(wiki_root: Option<&Path>) -> PathBuf {
    if let Some(root) = wiki_root {
        root.join(VAULT_FIXER_REPORT_DIR)
    } else {
        PathBuf::from(DEFAULT_FIXER_REPORT_DIR)
    }
}

pub(crate) fn default_synthesis_report_dir(wiki_root: Option<&Path>) -> PathBuf {
    if let Some(root) = wiki_root {
        root.join(VAULT_SYNTHESIS_REPORT_DIR)
    } else {
        PathBuf::from(DEFAULT_SYNTHESIS_REPORT_DIR)
    }
}

pub(crate) fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

// --- CLI argument parsing ---

pub(crate) fn parse_scope(s: &str) -> Scope {
    if let Some(x) = s.strip_prefix("shared:") {
        Scope::Shared {
            team_id: x.to_string(),
        }
    } else if let Some(x) = s.strip_prefix("private:") {
        Scope::Private {
            agent_id: x.to_string(),
        }
    } else {
        Scope::Private {
            agent_id: s.to_string(),
        }
    }
}

pub(crate) fn parse_tier(s: &str) -> Result<MemoryTier, Box<dyn std::error::Error>> {
    let x = s.trim().to_ascii_lowercase();
    match x.as_str() {
        "working" => Ok(MemoryTier::Working),
        "episodic" => Ok(MemoryTier::Episodic),
        "semantic" => Ok(MemoryTier::Semantic),
        "procedural" => Ok(MemoryTier::Procedural),
        _ => Err(format!("unknown tier: {s}").into()),
    }
}

/// 解析可选的 --entry-type 参数，使用 schema 的 strict parse。
pub(crate) fn parse_entry_type_opt(
    s: &Option<String>,
) -> Result<Option<EntryType>, Box<dyn std::error::Error>> {
    match s {
        Some(raw) => {
            let et = EntryType::parse(raw)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            Ok(Some(et))
        }
        None => Ok(None),
    }
}

/// ingest-llm 场景下的 entry_type 缺省策略：未指定时回退为 Concept。
/// 其它入口（crystallize / draft-from-query）保留 None 语义以避免意外写死。
#[allow(dead_code)]
pub(crate) fn effective_ingest_entry_type(explicit: Option<EntryType>) -> EntryType {
    explicit.unwrap_or(EntryType::Concept)
}

pub(crate) fn timestamp_slug() -> String {
    let now = OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "now".to_string())
        .replace(':', "-")
}

// --- Shared constants ---

pub(crate) const DEFAULT_MEMPALACE_CONSUMER_TAG: &str = "mempalace";
pub(crate) const DEFAULT_WRITER_LEASE_TTL_SECS: i64 = 6 * 60 * 60;
pub(crate) const DEFAULT_SCHEDULED_REPORT_KEEP: usize = 14;

// --- String utilities ---

pub(crate) fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
