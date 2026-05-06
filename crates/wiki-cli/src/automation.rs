use std::io::Write;
use std::path::Path;
use time::{Duration, OffsetDateTime};
use wiki_storage::{
    AutomationJobFailureSummary, AutomationRunRecord, AutomationRunStatus,
    OutboxConsumerProgress, OutboxStats, SqliteRepository, SqliteWriterLease, WikiRepository,
    WikiStateRowVerification,
};

use crate::cli_utils::{
    env_or, truncate_chars, DEFAULT_SCHEDULED_REPORT_KEEP,
    DEFAULT_WRITER_LEASE_TTL_SECS,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum AutomationJob {
    #[value(name = "notion-sync")]
    NotionSync,
    #[value(name = "batch-ingest")]
    BatchIngest,
    #[value(name = "governance-scan")]
    GovernanceScan,
    #[value(name = "fixer-plan")]
    FixerPlan,
    #[value(name = "fixer-apply")]
    FixerApply,
    #[value(name = "lint")]
    Lint,
    #[value(name = "maintenance")]
    Maintenance,
    #[value(name = "consume-to-mempalace")]
    ConsumeToMempalace,
    #[value(name = "llm-smoke")]
    LlmSmoke,
    #[value(name = "vault-reports")]
    VaultReports,
    #[value(name = "synthesis-discover")]
    SynthesisDiscover,
    #[value(name = "synthesis-run")]
    SynthesisRun,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomationJobSpec {
    pub(crate) job: AutomationJob,
    pub(crate) in_daily: bool,
    pub(crate) requires_network: bool,
    pub(crate) short_circuit: bool,
    pub(crate) description: &'static str,
}

const AUTOMATION_JOB_SPECS: &[AutomationJobSpec] = &[
    AutomationJobSpec {
        job: AutomationJob::NotionSync,
        in_daily: true,
        requires_network: true,
        short_circuit: false,
        description: "Incrementally sync Notion databases (X书签 and 微信文章) into wiki.db.",
    },
    AutomationJobSpec {
        job: AutomationJob::BatchIngest,
        in_daily: true,
        requires_network: true,
        short_circuit: true,
        description: "Compile vault sources with compiled_to_wiki=false into wiki.db.",
    },
    AutomationJobSpec {
        job: AutomationJob::GovernanceScan,
        in_daily: true,
        requires_network: false,
        short_circuit: true,
        description: "Write a read-only governance scan report for lifecycle, references, duplicates, and synthesis signals.",
    },
    AutomationJobSpec {
        job: AutomationJob::FixerPlan,
        in_daily: true,
        requires_network: true,
        short_circuit: true,
        description: "Build an evidence fixer plan from the current governance scan evidence.",
    },
    AutomationJobSpec {
        job: AutomationJob::FixerApply,
        in_daily: true,
        requires_network: false,
        short_circuit: true,
        description: "Apply ready evidence-auto fixer actions from the latest fixer plan.",
    },
    AutomationJobSpec {
        job: AutomationJob::Lint,
        in_daily: false,
        requires_network: false,
        short_circuit: true,
        description: "Run lint and write the latest report / projection outputs.",
    },
    AutomationJobSpec {
        job: AutomationJob::Maintenance,
        in_daily: true,
        requires_network: false,
        short_circuit: true,
        description: "Apply decay, lint, and auto-promote qualified claims/pages.",
    },
    AutomationJobSpec {
        job: AutomationJob::ConsumeToMempalace,
        in_daily: true,
        requires_network: false,
        short_circuit: true,
        description: "Replay outbox increments into palace.db and ack consumer progress.",
    },
    AutomationJobSpec {
        job: AutomationJob::LlmSmoke,
        in_daily: false,
        requires_network: true,
        short_circuit: true,
        description: "Check the configured LLM endpoint with a minimal chat completion.",
    },
    AutomationJobSpec {
        job: AutomationJob::VaultReports,
        in_daily: true,
        requires_network: false,
        short_circuit: false,
        description: "Generate scheduled Vault reports, latest pointers, and retention cleanup.",
    },
    AutomationJobSpec {
        job: AutomationJob::SynthesisDiscover,
        in_daily: false,
        requires_network: false,
        short_circuit: true,
        description: "Discover point/line/plane/volume synthesis candidates from internal wiki signals.",
    },
    AutomationJobSpec {
        job: AutomationJob::SynthesisRun,
        in_daily: false,
        requires_network: true,
        short_circuit: true,
        description: "Run synthesis discovery and compose verified in-review synthesis pages.",
    },
];

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AutomationHealthLevel {
    Green,
    Yellow,
    Red,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomationHealthThresholds {
    pub(crate) stale_heartbeat_yellow: Duration,
    pub(crate) stale_heartbeat_red: Duration,
    pub(crate) consecutive_failures_yellow: usize,
    pub(crate) consecutive_failures_red: usize,
    pub(crate) backlog_yellow: i64,
    pub(crate) backlog_red: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomationHealthIssue {
    pub(crate) level: AutomationHealthLevel,
    pub(crate) target: String,
    pub(crate) code: &'static str,
    pub(crate) detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AutomationHealthReport {
    pub(crate) level: AutomationHealthLevel,
    pub(crate) issues: Vec<AutomationHealthIssue>,
    pub(crate) db_integrity: String,
    pub(crate) outbox: OutboxStats,
    pub(crate) progress: OutboxConsumerProgress,
    pub(crate) failures: Vec<AutomationJobFailureSummary>,
}

pub(crate) struct AutomationHeartbeat<'a> {
    pub(crate) repo: &'a SqliteRepository,
    pub(crate) run_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestoreVaultSummary {
    pub(crate) pages: usize,
    pub(crate) sources: usize,
    pub(crate) frontmatter_checked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestorePalaceSummary {
    pub(crate) drawers: i64,
    pub(crate) kg_facts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestoreVerifyReport {
    pub(crate) outbox_head_id: i64,
    pub(crate) total_events: i64,
    pub(crate) vault: RestoreVaultSummary,
    pub(crate) palace: Option<RestorePalaceSummary>,
    pub(crate) progress: Option<OutboxConsumerProgress>,
}

// ---------------------------------------------------------------------------
// Job registry helpers
// ---------------------------------------------------------------------------

pub(crate) fn automation_health_thresholds() -> AutomationHealthThresholds {
    AutomationHealthThresholds {
        stale_heartbeat_yellow: Duration::hours(env_or("WIKI_HEALTH_STALE_YELLOW_HOURS", 6)),
        stale_heartbeat_red: Duration::hours(env_or("WIKI_HEALTH_STALE_RED_HOURS", 24)),
        consecutive_failures_yellow: env_or("WIKI_HEALTH_FAIL_YELLOW", 2),
        consecutive_failures_red: env_or("WIKI_HEALTH_FAIL_RED", 3),
        backlog_yellow: env_or("WIKI_HEALTH_BACKLOG_YELLOW", 25),
        backlog_red: env_or("WIKI_HEALTH_BACKLOG_RED", 100),
    }
}

pub(crate) fn automation_job_specs() -> &'static [AutomationJobSpec] {
    AUTOMATION_JOB_SPECS
}

pub(crate) fn automation_job_spec(job: AutomationJob) -> &'static AutomationJobSpec {
    automation_job_specs()
        .iter()
        .find(|spec| spec.job == job)
        .expect("automation job must exist in registry")
}

pub(crate) fn automation_all_jobs() -> Vec<AutomationJob> {
    automation_job_specs().iter().map(|spec| spec.job).collect()
}

pub(crate) fn automation_run_daily_jobs() -> Vec<AutomationJob> {
    automation_job_specs()
        .iter()
        .filter(|spec| spec.in_daily)
        .map(|spec| spec.job)
        .collect()
}

pub(crate) fn automation_job_name(job: AutomationJob) -> &'static str {
    match job {
        AutomationJob::NotionSync => "notion-sync",
        AutomationJob::BatchIngest => "batch-ingest",
        AutomationJob::GovernanceScan => "governance-scan",
        AutomationJob::FixerPlan => "fixer-plan",
        AutomationJob::FixerApply => "fixer-apply",
        AutomationJob::Lint => "lint",
        AutomationJob::Maintenance => "maintenance",
        AutomationJob::ConsumeToMempalace => "consume-to-mempalace",
        AutomationJob::LlmSmoke => "llm-smoke",
        AutomationJob::VaultReports => "vault-reports",
        AutomationJob::SynthesisDiscover => "synthesis-discover",
        AutomationJob::SynthesisRun => "synthesis-run",
    }
}

// ---------------------------------------------------------------------------
// Display / formatting helpers
// ---------------------------------------------------------------------------

pub(crate) fn print_automation_jobs<W: Write>(out: &mut W) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(out, "automation jobs:")?;
    for spec in automation_job_specs() {
        writeln!(
            out,
            "- {} daily={} requires_network={} :: {}",
            automation_job_name(spec.job),
            if spec.in_daily { "yes" } else { "no" },
            if spec.requires_network { "yes" } else { "no" },
            spec.description
        )?;
    }
    Ok(())
}

pub(crate) fn format_automation_time(value: OffsetDateTime) -> String {
    value
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}

pub(crate) fn format_automation_record(record: &AutomationRunRecord) -> String {
    let mut parts = vec![
        format!("status={:?}", record.status).to_lowercase(),
        format!("started_at={}", format_automation_time(record.started_at)),
        format!(
            "heartbeat_at={}",
            format_automation_time(record.heartbeat_at)
        ),
    ];
    if let Some(finished_at) = record.finished_at {
        parts.push(format!(
            "finished_at={}",
            format_automation_time(finished_at)
        ));
    }
    if let Some(duration_ms) = record.duration_ms {
        parts.push(format!("duration_ms={duration_ms}"));
    }
    if let Some(error_summary) = &record.error_summary {
        parts.push(format!(
            "error_summary={}",
            truncate_chars(error_summary, 160)
        ));
    }
    parts.join(" ")
}

pub(crate) fn format_outbox_stats(stats: &OutboxStats) -> String {
    format!(
        "head_id={} total_events={} unprocessed_events={}",
        stats.head_id, stats.total_events, stats.unprocessed_events
    )
}

pub(crate) fn format_outbox_consumer_progress(progress: &OutboxConsumerProgress) -> String {
    let mut parts = Vec::new();
    match progress.acked_up_to_id {
        Some(id) => parts.push(format!("acked_up_to_id={id}")),
        None => parts.push("acked_up_to_id=never".to_string()),
    }
    match progress.acked_at {
        Some(ts) => parts.push(format!("acked_at={}", format_automation_time(ts))),
        None => parts.push("acked_at=never".to_string()),
    }
    parts.push(format!("backlog_events={}", progress.backlog_events));
    parts.join(" ")
}

pub(crate) fn automation_health_level_name(level: AutomationHealthLevel) -> &'static str {
    match level {
        AutomationHealthLevel::Green => "green",
        AutomationHealthLevel::Yellow => "yellow",
        AutomationHealthLevel::Red => "red",
    }
}

pub(crate) fn max_health_level(a: AutomationHealthLevel, b: AutomationHealthLevel) -> AutomationHealthLevel {
    a.max(b)
}

fn format_duration_compact(duration: Duration) -> String {
    let secs = duration.whole_seconds().max(0);
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{hours}h{minutes}m{seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m{seconds}s")
    } else {
        format!("{seconds}s")
    }
}

// ---------------------------------------------------------------------------
// Health classification
// ---------------------------------------------------------------------------

pub(crate) fn classify_consecutive_failures(
    consecutive_failures: usize,
    thresholds: AutomationHealthThresholds,
) -> AutomationHealthLevel {
    if consecutive_failures >= thresholds.consecutive_failures_red {
        AutomationHealthLevel::Red
    } else if consecutive_failures >= thresholds.consecutive_failures_yellow {
        AutomationHealthLevel::Yellow
    } else {
        AutomationHealthLevel::Green
    }
}

pub(crate) fn classify_backlog(
    backlog_events: i64,
    thresholds: AutomationHealthThresholds,
) -> AutomationHealthLevel {
    if backlog_events >= thresholds.backlog_red {
        AutomationHealthLevel::Red
    } else if backlog_events >= thresholds.backlog_yellow {
        AutomationHealthLevel::Yellow
    } else {
        AutomationHealthLevel::Green
    }
}

pub(crate) fn classify_stale_heartbeat(
    record: &AutomationRunRecord,
    now: OffsetDateTime,
    thresholds: AutomationHealthThresholds,
) -> AutomationHealthLevel {
    if record.status != AutomationRunStatus::Running {
        return AutomationHealthLevel::Green;
    }
    let age = now - record.heartbeat_at;
    if age >= thresholds.stale_heartbeat_red {
        AutomationHealthLevel::Red
    } else if age >= thresholds.stale_heartbeat_yellow {
        AutomationHealthLevel::Yellow
    } else {
        AutomationHealthLevel::Green
    }
}

// ---------------------------------------------------------------------------
// Health report collection and rendering
// ---------------------------------------------------------------------------

pub(crate) fn collect_automation_health_report(
    repo: &SqliteRepository,
    jobs: &[AutomationJob],
    consumer_tag: &str,
    now: OffsetDateTime,
) -> Result<AutomationHealthReport, Box<dyn std::error::Error>> {
    let thresholds = automation_health_thresholds();
    let mut issues = Vec::new();
    let mut level = AutomationHealthLevel::Green;

    for job in jobs {
        let job_name = automation_job_name(*job);
        if let Some(record) = repo.get_latest_automation_run(job_name)? {
            let stale_level = classify_stale_heartbeat(&record, now, thresholds);
            if stale_level != AutomationHealthLevel::Green {
                let age = now - record.heartbeat_at;
                issues.push(AutomationHealthIssue {
                    level: stale_level,
                    target: job_name.to_string(),
                    code: "stale-heartbeat",
                    detail: format!(
                        "latest run is still running and heartbeat age={}",
                        format_duration_compact(age)
                    ),
                });
                level = max_health_level(level, stale_level);
            }
        }

        let consecutive_failures = repo.count_consecutive_automation_run_failures(job_name)?;
        let failure_level = classify_consecutive_failures(consecutive_failures, thresholds);
        if failure_level != AutomationHealthLevel::Green {
            issues.push(AutomationHealthIssue {
                level: failure_level,
                target: job_name.to_string(),
                code: "consecutive-failures",
                detail: format!("consecutive_failures={consecutive_failures}"),
            });
            level = max_health_level(level, failure_level);
        }
    }

    let outbox = repo.get_outbox_stats()?;
    let progress = repo.get_outbox_consumer_progress(consumer_tag)?;
    let backlog_level = classify_backlog(progress.backlog_events, thresholds);
    if backlog_level != AutomationHealthLevel::Green {
        issues.push(AutomationHealthIssue {
            level: backlog_level,
            target: format!("consumer:{consumer_tag}"),
            code: "consumer-backlog",
            detail: format!("backlog_events={}", progress.backlog_events),
        });
        level = max_health_level(level, backlog_level);
    }

    let db_integrity = repo.integrity_check()?;
    if db_integrity != "ok" {
        issues.push(AutomationHealthIssue {
            level: AutomationHealthLevel::Red,
            target: "wiki.db".to_string(),
            code: "db-integrity",
            detail: format!("integrity_check={db_integrity}"),
        });
        level = AutomationHealthLevel::Red;
    }

    let failures = repo.list_automation_job_failure_summaries()?;
    Ok(AutomationHealthReport {
        level,
        issues,
        db_integrity,
        outbox,
        progress,
        failures,
    })
}

pub(crate) fn render_automation_health_report(report: &AutomationHealthReport, consumer_tag: &str) -> String {
    let thresholds = automation_health_thresholds();
    let mut out = String::new();
    out.push_str(&format!(
        "automation health: status={} consumer_tag={consumer_tag}\n",
        automation_health_level_name(report.level)
    ));
    out.push_str(&format!(
        "thresholds: stale_heartbeat_yellow={} stale_heartbeat_red={} consecutive_failures_yellow={} consecutive_failures_red={} backlog_yellow={} backlog_red={}\n",
        format_duration_compact(thresholds.stale_heartbeat_yellow),
        format_duration_compact(thresholds.stale_heartbeat_red),
        thresholds.consecutive_failures_yellow,
        thresholds.consecutive_failures_red,
        thresholds.backlog_yellow,
        thresholds.backlog_red,
    ));
    if report.issues.is_empty() {
        out.push_str("issues: none\n");
    } else {
        out.push_str("issues:\n");
        for issue in &report.issues {
            out.push_str(&format!(
                "- {} target={} code={} detail={}\n",
                automation_health_level_name(issue.level),
                issue.target,
                issue.code,
                issue.detail
            ));
        }
    }
    out.push_str(&format!(
        "outbox: {}\n",
        format_outbox_stats(&report.outbox)
    ));
    out.push_str(&format!("db_integrity: {}\n", report.db_integrity));
    out.push_str(&format!(
        "consumer {consumer_tag}: {}\n",
        format_outbox_consumer_progress(&report.progress)
    ));
    out.push_str("last_failures:\n");
    if report.failures.is_empty() {
        out.push_str("- none\n");
    } else {
        for failure in &report.failures {
            let detail = failure
                .latest_failure
                .as_ref()
                .map(format_automation_record)
                .unwrap_or_else(|| "latest_failure=missing".to_string());
            out.push_str(&format!(
                "- job={} consecutive_failures={} {}\n",
                failure.job_name, failure.consecutive_failures, detail
            ));
        }
    }
    let action = match report.level {
        AutomationHealthLevel::Green => "manual_action=no_intervention_required",
        AutomationHealthLevel::Yellow => "manual_action=review_warnings_before_next_daily_run",
        AutomationHealthLevel::Red => "manual_action=investigate_and_fix_before_next_daily_run",
    };
    out.push_str(action);
    out.push('\n');
    out
}

pub(crate) fn print_automation_last_failures<W: Write>(
    repo: &SqliteRepository,
    limit: usize,
    out: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(out, "automation last-failures:")?;
    let failures = repo.list_recent_failed_automation_runs(limit)?;
    if failures.is_empty() {
        writeln!(out, "- none")?;
        return Ok(());
    }
    for record in failures {
        writeln!(
            out,
            "- job={} {}",
            record.job_name,
            format_automation_record(&record)
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Automation plan execution
// ---------------------------------------------------------------------------

pub(crate) fn run_automation_plan<W, F>(
    jobs: &[AutomationJob],
    dry_run: bool,
    out: &mut W,
    mut run_job: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    W: Write,
    F: FnMut(AutomationJob) -> Result<(), Box<dyn std::error::Error>>,
{
    writeln!(out, "automation run-daily plan:")?;
    for (idx, job) in jobs.iter().enumerate() {
        writeln!(out, "{}. {}", idx + 1, automation_job_name(*job))?;
    }

    if dry_run {
        writeln!(out, "dry-run: no jobs executed")?;
        return Ok(());
    }

    let mut first_failure: Option<String> = None;
    for job in jobs {
        writeln!(out, "automation: running {}", automation_job_name(*job))?;
        if let Err(err) = run_job(*job) {
            let spec = automation_job_spec(*job);
            let msg = err.to_string();
            writeln!(
                out,
                "automation: failed {} with {}",
                automation_job_name(*job),
                msg
            )?;
            if spec.short_circuit {
                return Err(err);
            }
            if first_failure.is_none() {
                first_failure = Some(msg);
            }
            continue;
        }

        writeln!(out, "automation: finished {}", automation_job_name(*job))?;
    }

    if let Some(msg) = first_failure {
        return Err(Box::new(std::io::Error::other(msg)));
    }
    Ok(())
}

pub(crate) fn print_automation_status<W: Write>(
    repo: &SqliteRepository,
    jobs: &[AutomationJob],
    out: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(out, "automation status:")?;
    for job in jobs {
        let job_name = automation_job_name(*job);
        let latest = repo.get_latest_automation_run(job_name)?;
        match latest {
            Some(record) => {
                writeln!(out, "{job_name}: {}", format_automation_record(&record))?;
            }
            None => {
                writeln!(out, "{job_name}: never-run")?;
            }
        }
    }
    Ok(())
}

pub(crate) fn print_automation_doctor<W: Write>(
    repo: &SqliteRepository,
    jobs: &[AutomationJob],
    consumer_tag: &str,
    out: &mut W,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(out, "automation doctor:")?;
    for job in jobs {
        let job_name = automation_job_name(*job);
        let latest = repo.get_latest_automation_run(job_name)?;
        match latest {
            Some(record) => {
                writeln!(out, "{job_name}: {}", format_automation_record(&record))?;
            }
            None => {
                writeln!(out, "{job_name}: never-run")?;
            }
        }
    }
    let outbox = repo.get_outbox_stats()?;
    writeln!(out, "outbox: {}", format_outbox_stats(&outbox))?;
    let progress = repo.get_outbox_consumer_progress(consumer_tag)?;
    writeln!(
        out,
        "consumer {consumer_tag}: {}",
        format_outbox_consumer_progress(&progress)
    )?;
    Ok(())
}

pub(crate) fn emit_automation_health_alert(level: AutomationHealthLevel) {
    match level {
        AutomationHealthLevel::Green => {}
        AutomationHealthLevel::Yellow => {
            eprintln!("\x1b[33mALERT YELLOW\x1b[0m automation health requires review");
        }
        AutomationHealthLevel::Red => {
            eprintln!("\x1b[31mALERT RED\x1b[0m automation health requires intervention");
        }
    }
}

// ---------------------------------------------------------------------------
// Scheduled report retention
// ---------------------------------------------------------------------------

pub(crate) fn scheduled_report_keep_count() -> usize {
    env_or("WIKI_SCHEDULED_REPORT_KEEP", DEFAULT_SCHEDULED_REPORT_KEEP).max(1)
}

pub(crate) fn path_for_report(path: &Path) -> String {
    path.display().to_string()
}

pub(crate) fn prune_scheduled_report_runs(root: &Path, keep: usize) -> std::io::Result<usize> {
    if !root.exists() {
        return Ok(0);
    }

    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.ends_with("-scheduled") {
            dirs.push((name.to_string(), path));
        }
    }
    dirs.sort_by(|a, b| b.0.cmp(&a.0));

    let mut pruned = 0;
    for (_, path) in dirs.into_iter().skip(keep) {
        std::fs::remove_dir_all(path)?;
        pruned += 1;
    }
    Ok(pruned)
}

pub(crate) fn scheduled_report_timestamp(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

// ---------------------------------------------------------------------------
// Restore verification
// ---------------------------------------------------------------------------

pub(crate) fn ensure_sqlite_integrity(db_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if !db_path.is_file() {
        return Err(format!("wiki.db 不存在: {}", db_path.display()).into());
    }
    let conn = rusqlite::Connection::open(db_path)?;
    let integrity: String = conn.query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(format!(
            "wiki.db integrity_check 失败: {} ({})",
            integrity,
            db_path.display()
        )
        .into());
    }
    Ok(())
}

pub(crate) fn verify_restore_vault(
    wiki_root: &std::path::Path,
) -> Result<RestoreVaultSummary, Box<dyn std::error::Error>> {
    if !wiki_root.is_dir() {
        return Err(format!("wiki-dir 不存在: {}", wiki_root.display()).into());
    }
    for required in ["index.md", "log.md"] {
        let path = wiki_root.join(required);
        if !path.is_file() {
            return Err(format!("vault 缺少 {}", path.display()).into());
        }
    }

    let pages_dir = wiki_root.join("pages");
    if !pages_dir.is_dir() {
        return Err(format!("vault 缺少 pages/ 目录: {}", pages_dir.display()).into());
    }
    let sources_dir = wiki_root.join("sources");
    if !sources_dir.is_dir() {
        return Err(format!("vault 缺少 sources/ 目录: {}", sources_dir.display()).into());
    }

    let mut pages = 0usize;
    let mut frontmatter_checked = 0usize;
    for entry in walkdir::WalkDir::new(&pages_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        pages += 1;
        let content = std::fs::read_to_string(entry.path())?;
        let mut lines = content.lines();
        let first_line = lines.next().unwrap_or_default();
        if first_line != "---" {
            return Err(format!("frontmatter missing in {}", entry.path().display()).into());
        }
        if !content.lines().any(|line| line.starts_with("status:")) {
            return Err(format!("status field missing in {}", entry.path().display()).into());
        }
        frontmatter_checked += 1;
    }
    if pages == 0 {
        return Err(format!("vault pages/ 下没有 md 文件: {}", pages_dir.display()).into());
    }

    let sources = walkdir::WalkDir::new(&sources_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .count();
    if sources == 0 {
        return Err(format!("vault sources/ 下没有文件: {}", sources_dir.display()).into());
    }

    Ok(RestoreVaultSummary {
        pages,
        sources,
        frontmatter_checked,
    })
}

pub(crate) fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> Result<bool, rusqlite::Error> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
}

pub(crate) fn verify_restore_palace(
    palace_path: &std::path::Path,
) -> Result<RestorePalaceSummary, Box<dyn std::error::Error>> {
    if !palace_path.is_file() {
        return Err(format!("palace.db 不存在: {}", palace_path.display()).into());
    }
    let conn = rusqlite::Connection::open(palace_path)?;
    for table in ["drawers", "drawer_vectors", "kg_facts"] {
        if !table_exists(&conn, table)? {
            return Err(
                format!("palace.db 缺少核心表 {}: {}", table, palace_path.display()).into(),
            );
        }
    }
    let drawers: i64 = conn.query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))?;
    let kg_facts: i64 = conn.query_row("SELECT COUNT(*) FROM kg_facts", [], |row| row.get(0))?;
    Ok(RestorePalaceSummary { drawers, kg_facts })
}

pub(crate) fn collect_restore_verify_report(
    db_path: &std::path::Path,
    repo: &SqliteRepository,
    wiki_root: &std::path::Path,
    palace_path: Option<&std::path::Path>,
    consumer_tag: &str,
) -> Result<RestoreVerifyReport, Box<dyn std::error::Error>> {
    ensure_sqlite_integrity(db_path)?;
    let _snapshot = repo.load_snapshot()?;
    let outbox = repo.get_outbox_stats()?;
    let vault = verify_restore_vault(wiki_root)?;
    let palace = palace_path.map(verify_restore_palace).transpose()?;
    let progress = if palace_path.is_some() {
        Some(repo.get_outbox_consumer_progress(consumer_tag)?)
    } else {
        None
    };
    Ok(RestoreVerifyReport {
        outbox_head_id: outbox.head_id,
        total_events: outbox.total_events,
        vault,
        palace,
        progress,
    })
}

pub(crate) fn render_restore_verify_report(report: &RestoreVerifyReport, consumer_tag: &str) -> String {
    let mut out = String::new();
    out.push_str("restore verify: status=ok\n");
    out.push_str(&format!(
        "wiki_db: integrity=ok outbox_head_id={} total_events={}\n",
        report.outbox_head_id, report.total_events
    ));
    out.push_str(&format!(
        "vault: index=ok log=ok pages={} sources={} frontmatter_checked={}\n",
        report.vault.pages, report.vault.sources, report.vault.frontmatter_checked
    ));
    if let Some(palace) = &report.palace {
        let progress = report
            .progress
            .as_ref()
            .expect("palace progress should exist");
        let acked = progress
            .acked_up_to_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "never".to_string());
        out.push_str(&format!(
            "palace: status=ok drawers={} kg_facts={} consumer_tag={} acked_up_to_id={} backlog_events={}\n",
            palace.drawers, palace.kg_facts, consumer_tag, acked, progress.backlog_events
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Automation job execution wrapper
// ---------------------------------------------------------------------------

impl AutomationHeartbeat<'_> {
    pub(crate) fn tick(&self) {
        if let Some(id) = self.run_id {
            let _ = self.repo.refresh_automation_heartbeat(id);
        }
    }
}

pub(crate) fn run_automation_job<F>(
    repo: &SqliteRepository,
    job: AutomationJob,
    run: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&AutomationHeartbeat<'_>) -> Result<(), Box<dyn std::error::Error>>,
{
    let job_name = automation_job_name(job);
    let run_id = repo.start_automation_run(job_name)?;
    repo.refresh_automation_heartbeat(run_id)?;
    let hb = AutomationHeartbeat {
        repo,
        run_id: Some(run_id),
    };
    match run(&hb) {
        Ok(()) => {
            repo.mark_automation_run_succeeded(run_id)?;
            Ok(())
        }
        Err(err) => {
            let summary = truncate_chars(&err.to_string(), 240);
            if let Err(storage_err) = repo.mark_automation_run_failed(run_id, &summary) {
                return Err(format!(
                    "{job_name} failed: {summary}; additionally failed to persist run state: {storage_err}"
                )
                .into());
            }
            Err(err)
        }
    }
}

pub(crate) fn latest_automation_run_or_error(
    repo: &SqliteRepository,
    job: AutomationJob,
) -> Result<AutomationRunRecord, Box<dyn std::error::Error>> {
    repo.get_latest_automation_run(automation_job_name(job))?
        .ok_or_else(|| {
            format!(
                "missing automation run record for job {}",
                automation_job_name(job)
            )
            .into()
        })
}

pub(crate) fn writer_lease_ttl() -> Duration {
    std::env::var("WIKI_WRITER_LEASE_TTL_SECS")
        .ok()
        .and_then(|raw| raw.parse::<i64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::seconds)
        .unwrap_or_else(|| Duration::seconds(DEFAULT_WRITER_LEASE_TTL_SECS))
}

pub(crate) fn automation_job_needs_writer_lease(job: AutomationJob) -> bool {
    matches!(
        job,
        AutomationJob::BatchIngest
            | AutomationJob::FixerApply
            | AutomationJob::Lint
            | AutomationJob::Maintenance
            | AutomationJob::ConsumeToMempalace
            | AutomationJob::NotionSync
            | AutomationJob::SynthesisRun
    )
}

pub(crate) fn acquire_cli_writer_lease(
    db_path: &std::path::Path,
    label: &str,
) -> Result<SqliteWriterLease, Box<dyn std::error::Error>> {
    SqliteWriterLease::acquire(db_path, format!("wiki-cli:{label}"), writer_lease_ttl())
        .map_err(|err| -> Box<dyn std::error::Error> { err.to_string().into() })
}

// ---------------------------------------------------------------------------
// Row-state verification
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub(crate) struct RowStateVerificationOutput<'a> {
    pub(crate) status: &'a str,
    #[serde(flatten)]
    pub(crate) verification: &'a WikiStateRowVerification,
}

pub(crate) fn run_verify_row_state(
    db_path: &Path,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = SqliteRepository::open_read_only(db_path)?;
    let verification = repo.verify_row_state_matches_blob()?;
    let status = if row_state_verification_error(&verification).is_none() {
        "ok"
    } else {
        "error"
    };

    if json {
        let output = RowStateVerificationOutput {
            status,
            verification: &verification,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        print_row_state_verification(&verification, status);
    }

    if let Some(error) = row_state_verification_error(&verification) {
        return Err(error.into());
    }
    Ok(())
}

pub(crate) fn print_row_state_verification(verification: &WikiStateRowVerification, status: &str) {
    let matches_blob = match verification.matches_blob {
        Some(true) => "true",
        Some(false) => "false",
        None => "n/a",
    };
    println!(
        "row_state status={} rows={} blob_present={} matches_blob={}",
        status, verification.row_count, verification.blob_present, matches_blob
    );
    for count in &verification.collection_counts {
        println!(
            "row_state_collection collection={} rows={}",
            count.collection, count.row_count
        );
    }
}

pub(crate) fn row_state_verification_error(verification: &WikiStateRowVerification) -> Option<String> {
    if verification.row_count == 0 {
        return Some("row-level state has no rows; blob fallback is still required".to_string());
    }
    if !verification.blob_present {
        return Some(
            "legacy snapshot blob is missing; fallback compatibility cannot be verified"
                .to_string(),
        );
    }
    if verification.matches_blob != Some(true) {
        return Some("row-level state does not match legacy snapshot blob".to_string());
    }
    None
}
