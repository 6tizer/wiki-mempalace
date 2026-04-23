use clap::{Parser, Subcommand, ValueEnum};
use std::io::Write;
use std::path::PathBuf;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use walkdir::WalkDir;
use wiki_core::{
    document_visible_to_viewer, parse_memory_tier, ClaimId, DomainSchema, Entity, EntityId,
    EntityKind, EntryStatus, EntryType, GapFinding, GapSeverity, LlmIngestPlanV1, MemoryTier,
    PageId, QueryContext, RelationKind, Scope, SessionCrystallizationInput, SourceId, TypedEdge,
    WikiPage,
};
use wiki_kernel::{
    format_claim_doc_id, initial_status_for, merge_graph_rankings, write_lint_report,
    write_projection, InMemorySearchPorts, InMemoryStore, LlmWikiEngine, NoopWikiHook, SearchPorts,
};
use wiki_mempalace_bridge::{
    consume_outbox_ndjson_with_resolver_and_stats, LiveMempalaceSink, MempalaceError,
    MempalaceWikiSink, OutboxDispatchStats, OutboxResolver,
};
use wiki_storage::{
    AutomationJobFailureSummary, AutomationRunRecord, AutomationRunStatus, OutboxConsumerProgress,
    OutboxStats, SqliteRepository, WikiRepository,
};

mod banner;
mod llm;
mod mcp;

const DEFAULT_MEMPALACE_CONSUMER_TAG: &str = "mempalace";

#[derive(Parser)]
#[command(name = "wiki")]
#[command(
    about = "SQLite + Markdown wiki, RRF query, NDJSON outbox; optional embeddings & MemPalace hooks.",
    long_about = None
)]
struct Cli {
    #[arg(long, default_value = "wiki.db")]
    db: PathBuf,
    #[arg(long)]
    schema: Option<PathBuf>,
    #[arg(long)]
    wiki_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    sync_wiki: bool,
    /// 检索 / lint / promote 的视角 scope（多 agent 隔离）。例如 `private:cli` 或 `shared:team1`。
    #[arg(long, default_value = "private:cli")]
    viewer_scope: String,
    /// 使用 `llm-config.toml` 中 `[embed]` 做向量检索（需联网）。
    #[arg(long, default_value_t = false)]
    vectors: bool,
    #[arg(long, default_value = "llm-config.toml")]
    llm_config: PathBuf,
    /// 每行一个 `entity:` / `claim:` / `page:` doc id，与内核图路按轮次合并后作为 RRF 第三路。
    #[arg(long)]
    graph_extras_file: Option<PathBuf>,
    /// palace.db 路径（启用后 consume-to-mempalace 写入真实 palace 数据库）。
    #[arg(long)]
    palace: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Ingest {
        uri: String,
        body: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
    },
    IngestLlm {
        uri: String,
        body: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// 已废弃：自 M7 起 ingest-llm 产出的 summary page 固定为 `EntryType::Summary`，
        /// 传入此参数会打印一条 stderr 警告后被忽略。保留仅为避免旧脚本报 unknown argument。
        #[arg(long, hide = true)]
        entry_type: Option<String>,
    },
    FileClaim {
        text: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
        #[arg(long, default_value = "working")]
        tier: String,
    },
    SupersedeClaim {
        old_claim_id: String,
        new_text: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
        #[arg(long, default_value = "working")]
        tier: String,
    },
    Query {
        query: String,
        #[arg(long, default_value_t = 60.0)]
        rrf_k: f64,
        #[arg(long, default_value_t = 50)]
        per_stream_limit: usize,
        #[arg(long, default_value_t = false)]
        write_page: bool,
        #[arg(long)]
        page_title: Option<String>,
        /// 为 query 生成的 page 绑定 EntryType（如 concept、entity、qa）。
        #[arg(long)]
        entry_type: Option<String>,
    },
    Lint,
    /// 检测知识缺口并生成 gap 报告。
    Gap {
        /// 低覆盖阈值：关联 claim 数量少于此值的 entity 会被标记。
        #[arg(long, default_value_t = 2)]
        low_coverage_threshold: usize,
        /// 将 gap 报告写入 wiki page（draft 状态）。
        #[arg(long, default_value_t = false)]
        write_page: bool,
    },
    Promote {
        claim_id: String,
    },
    /// Promote a page's lifecycle status (Draft → InReview → Approved).
    PromotePage {
        page_id: String,
        /// Target status. If omitted, auto-advance to the next status per lifecycle rule.
        #[arg(long)]
        to: Option<String>,
        /// Skip all promotion condition checks.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    Crystallize {
        question: String,
        #[arg(long = "finding")]
        findings: Vec<String>,
        #[arg(long = "file")]
        files: Vec<String>,
        #[arg(long = "lesson")]
        lessons: Vec<String>,
        /// 为 crystallize 生成的 page 绑定 EntryType。
        #[arg(long)]
        entry_type: Option<String>,
    },
    ExportOutboxNdjson,
    ExportOutboxNdjsonFrom {
        #[arg(long, default_value_t = 0)]
        last_id: i64,
    },
    AckOutbox {
        #[arg(long)]
        up_to_id: i64,
        #[arg(long)]
        consumer_tag: String,
    },
    ConsumeToMempalace {
        /// 最小 outbox id；实际起点取 consumer progress 与此值中的较大者。
        #[arg(long, default_value_t = 0)]
        last_id: i64,
        /// 用于 outbox ack / progress 跟踪的 consumer tag。
        #[arg(long, default_value = "mempalace")]
        consumer_tag: String,
    },
    LlmSmoke {
        #[arg(long, default_value = "llm-config.toml")]
        config: PathBuf,
        #[arg(long, default_value = "Say 'ok' only.")]
        prompt: String,
    },
    /// Start a unified MCP server (wiki + mempalace) over stdin/stdout.
    Mcp {
        #[arg(long, default_value_t = false)]
        once: bool,
    },
    /// Validate a DomainSchema JSON file and print summary.
    SchemaValidate {
        /// JSON 文件路径，默认 DomainSchema.json
        path: Option<PathBuf>,
    },
    /// Run batch maintenance: confidence decay, lint, promote qualified claims.
    Maintenance,
    /// 批量编译 vault 中 compiled_to_wiki: false 的 source 文件（调用 LLM 抽取后写入引擎）
    BatchIngest {
        /// vault 根目录（含 sources/）；默认取 $WIKI_VAULT_DIR 或 ~/Documents/wiki
        #[arg(long)]
        vault: Option<PathBuf>,
        /// 限制处理条数（用于测试）
        #[arg(long)]
        limit: Option<usize>,
        /// 只扫描不编译，输出待处理列表
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// 每条之间休眠秒数（避免 LLM 限流）
        #[arg(long, default_value_t = 1)]
        delay_secs: u64,
    },
    /// Run, inspect, and monitor scheduled automation jobs.
    Automation {
        #[command(subcommand)]
        cmd: AutomationCmd,
    },
}

#[derive(Subcommand)]
enum AutomationCmd {
    /// List all registered automation jobs and their execution semantics.
    ListJobs,
    /// Run the fixed daily automation chain: batch-ingest, lint, maintenance, consume-to-mempalace.
    RunDaily {
        /// Print the execution plan without running any jobs.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Run a single named automation job.
    Run {
        #[arg(value_enum)]
        job: AutomationJob,
    },
    /// Print the most recent failed automation runs across all jobs.
    LastFailures {
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Print the latest automation run status for each registered job.
    Status,
    /// Print job status plus outbox / consumer health summary.
    Doctor {
        /// Consumer tag used for outbox ack / lag tracking.
        #[arg(long, default_value = "mempalace")]
        consumer_tag: String,
    },
    /// Evaluate health thresholds and emit alert-friendly output.
    Health {
        /// Consumer tag used for outbox ack / lag tracking.
        #[arg(long, default_value = "mempalace")]
        consumer_tag: String,
        /// Optional local summary file path for operators / cron hooks.
        #[arg(long)]
        summary_file: Option<PathBuf>,
        /// Exit with code 1 on Yellow or Red (useful for CI / cron alerting).
        #[arg(long, default_value_t = false)]
        exit_on_yellow: bool,
    },
    /// Verify that a restored wiki.db / vault / optional palace.db is structurally healthy.
    VerifyRestore,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum AutomationJob {
    #[value(name = "batch-ingest")]
    BatchIngest,
    #[value(name = "lint")]
    Lint,
    #[value(name = "maintenance")]
    Maintenance,
    #[value(name = "consume-to-mempalace")]
    ConsumeToMempalace,
    #[value(name = "llm-smoke")]
    LlmSmoke,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct AutomationJobSpec {
    job: AutomationJob,
    in_daily: bool,
    requires_network: bool,
    description: &'static str,
}

const AUTOMATION_JOB_SPECS: &[AutomationJobSpec] = &[
    AutomationJobSpec {
        job: AutomationJob::BatchIngest,
        in_daily: true,
        requires_network: true,
        description: "Compile vault sources with compiled_to_wiki=false into wiki.db.",
    },
    AutomationJobSpec {
        job: AutomationJob::Lint,
        in_daily: true,
        requires_network: false,
        description: "Run lint and write the latest report / projection outputs.",
    },
    AutomationJobSpec {
        job: AutomationJob::Maintenance,
        in_daily: true,
        requires_network: false,
        description: "Apply decay, lint, and auto-promote qualified claims/pages.",
    },
    AutomationJobSpec {
        job: AutomationJob::ConsumeToMempalace,
        in_daily: true,
        requires_network: false,
        description: "Replay outbox increments into palace.db and ack consumer progress.",
    },
    AutomationJobSpec {
        job: AutomationJob::LlmSmoke,
        in_daily: false,
        requires_network: true,
        description: "Check the configured LLM endpoint with a minimal chat completion.",
    },
];

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AutomationHealthLevel {
    Green,
    Yellow,
    Red,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct AutomationHealthThresholds {
    stale_heartbeat_yellow: Duration,
    stale_heartbeat_red: Duration,
    consecutive_failures_yellow: usize,
    consecutive_failures_red: usize,
    backlog_yellow: i64,
    backlog_red: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutomationHealthIssue {
    level: AutomationHealthLevel,
    target: String,
    code: &'static str,
    detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AutomationHealthReport {
    level: AutomationHealthLevel,
    issues: Vec<AutomationHealthIssue>,
    outbox: OutboxStats,
    progress: OutboxConsumerProgress,
    failures: Vec<AutomationJobFailureSummary>,
}

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn automation_health_thresholds() -> AutomationHealthThresholds {
    AutomationHealthThresholds {
        stale_heartbeat_yellow: Duration::hours(env_or("WIKI_HEALTH_STALE_YELLOW_HOURS", 6)),
        stale_heartbeat_red: Duration::hours(env_or("WIKI_HEALTH_STALE_RED_HOURS", 24)),
        consecutive_failures_yellow: env_or("WIKI_HEALTH_FAIL_YELLOW", 2),
        consecutive_failures_red: env_or("WIKI_HEALTH_FAIL_RED", 3),
        backlog_yellow: env_or("WIKI_HEALTH_BACKLOG_YELLOW", 25),
        backlog_red: env_or("WIKI_HEALTH_BACKLOG_RED", 100),
    }
}

fn automation_job_specs() -> &'static [AutomationJobSpec] {
    AUTOMATION_JOB_SPECS
}

fn automation_job_spec(job: AutomationJob) -> &'static AutomationJobSpec {
    automation_job_specs()
        .iter()
        .find(|spec| spec.job == job)
        .expect("automation job must exist in registry")
}

fn automation_all_jobs() -> Vec<AutomationJob> {
    automation_job_specs().iter().map(|spec| spec.job).collect()
}

fn automation_run_daily_jobs() -> Vec<AutomationJob> {
    automation_job_specs()
        .iter()
        .filter(|spec| spec.in_daily)
        .map(|spec| spec.job)
        .collect()
}

fn automation_job_name(job: AutomationJob) -> &'static str {
    match job {
        AutomationJob::BatchIngest => "batch-ingest",
        AutomationJob::Lint => "lint",
        AutomationJob::Maintenance => "maintenance",
        AutomationJob::ConsumeToMempalace => "consume-to-mempalace",
        AutomationJob::LlmSmoke => "llm-smoke",
    }
}

fn print_automation_jobs<W: Write>(out: &mut W) -> Result<(), Box<dyn std::error::Error>> {
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

fn format_automation_time(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}

fn format_automation_record(record: &AutomationRunRecord) -> String {
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

fn format_outbox_stats(stats: &OutboxStats) -> String {
    format!(
        "head_id={} total_events={} unprocessed_events={}",
        stats.head_id, stats.total_events, stats.unprocessed_events
    )
}

fn format_outbox_consumer_progress(progress: &OutboxConsumerProgress) -> String {
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

fn automation_health_level_name(level: AutomationHealthLevel) -> &'static str {
    match level {
        AutomationHealthLevel::Green => "green",
        AutomationHealthLevel::Yellow => "yellow",
        AutomationHealthLevel::Red => "red",
    }
}

fn max_health_level(a: AutomationHealthLevel, b: AutomationHealthLevel) -> AutomationHealthLevel {
    a.max(b)
}

fn classify_consecutive_failures(
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

fn classify_backlog(
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

fn classify_stale_heartbeat(
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

fn collect_automation_health_report(
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

    let failures = repo.list_automation_job_failure_summaries()?;
    Ok(AutomationHealthReport {
        level,
        issues,
        outbox,
        progress,
        failures,
    })
}

fn render_automation_health_report(report: &AutomationHealthReport, consumer_tag: &str) -> String {
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
                .map(|run| format_automation_record(run))
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

fn print_automation_last_failures<W: Write>(
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

fn run_automation_plan<W, F>(
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

    for job in jobs {
        writeln!(out, "automation: running {}", automation_job_name(*job))?;
        run_job(*job)?;
        writeln!(out, "automation: finished {}", automation_job_name(*job))?;
    }

    Ok(())
}

fn print_automation_status<W: Write>(
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

fn print_automation_doctor<W: Write>(
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

fn emit_automation_health_alert(level: AutomationHealthLevel) {
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

struct AutomationHeartbeat<'a> {
    repo: &'a SqliteRepository,
    run_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestoreVaultSummary {
    pages: usize,
    sources: usize,
    frontmatter_checked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestorePalaceSummary {
    drawers: i64,
    kg_facts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestoreVerifyReport {
    outbox_head_id: i64,
    total_events: i64,
    vault: RestoreVaultSummary,
    palace: Option<RestorePalaceSummary>,
    progress: Option<OutboxConsumerProgress>,
}

fn ensure_sqlite_integrity(db_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
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

fn verify_restore_vault(
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
    for entry in WalkDir::new(&pages_dir).into_iter().filter_map(Result::ok) {
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

    let sources = WalkDir::new(&sources_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .count();

    Ok(RestoreVaultSummary {
        pages,
        sources,
        frontmatter_checked,
    })
}

fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> Result<bool, rusqlite::Error> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
}

fn verify_restore_palace(
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

fn collect_restore_verify_report(
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

fn render_restore_verify_report(report: &RestoreVerifyReport, consumer_tag: &str) -> String {
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

impl AutomationHeartbeat<'_> {
    fn tick(&self) {
        if let Some(id) = self.run_id {
            let _ = self.repo.refresh_automation_heartbeat(id);
        }
    }
}

fn run_automation_job<F>(
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

fn latest_automation_run_or_error(
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = match Cli::try_parse() {
        Ok(v) => v,
        Err(e) => {
            banner::print_startup_banner();
            e.print()?;
            std::process::exit(if e.use_stderr() { 2 } else { 0 });
        }
    };
    if !matches!(cli.cmd, Cmd::Mcp { .. } | Cmd::SchemaValidate { .. }) {
        banner::print_startup_banner();
    }

    // SchemaValidate 不需要 DB / engine，直接短路
    if let Cmd::SchemaValidate { path } = cli.cmd {
        let p = path.unwrap_or_else(|| PathBuf::from("DomainSchema.json"));
        match DomainSchema::from_json_path(&p) {
            Ok(schema) => {
                println!(
                    "schema ok: title={} lifecycle_rules={}",
                    schema.title,
                    schema.lifecycle_rules.len()
                );
                Ok(())
            }
            Err(e) => {
                eprintln!("schema invalid: {e}");
                std::process::exit(1);
            }
        }
    } else {
        run_with_engine(cli)
    }
}

/// 所有需要 DB / engine 的子命令走这里。
fn run_with_engine(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    if matches!(
        &cli.cmd,
        Cmd::Automation {
            cmd: AutomationCmd::ListJobs
        }
    ) {
        let mut stdout = std::io::stdout().lock();
        print_automation_jobs(&mut stdout)?;
        return Ok(());
    }

    if matches!(
        &cli.cmd,
        Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: true }
        }
    ) {
        let jobs = automation_run_daily_jobs();
        let mut stdout = std::io::stdout().lock();
        run_automation_plan(&jobs, true, &mut stdout, |_| Ok(()))?;
        return Ok(());
    }

    let viewer = parse_scope(&cli.viewer_scope);
    let wiki_root = cli.wiki_dir.clone();
    let sync_wiki = cli.sync_wiki;
    let repo = SqliteRepository::open(&cli.db)?;
    let schema = if let Some(path) = &cli.schema {
        DomainSchema::from_json_path(path)?
    } else {
        DomainSchema::permissive_default()
    };
    let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook)?;

    match cli.cmd {
        Cmd::IngestLlm {
            uri,
            body,
            scope,
            dry_run,
            entry_type,
        } => {
            if entry_type.is_some() {
                eprintln!(
                    "warning: --entry-type on `ingest-llm` is deprecated since M7 and is ignored; \
                     all ingest-llm summary pages are fixed to EntryType::Summary."
                );
            }
            let cfg = llm::load_llm_config(&cli.llm_config)?;
            let user = format!("Source URI:\n{uri}\n\nBody:\n{body}");
            let reply = llm::complete_chat(&cfg, llm::ingest_llm_system_prompt(), &user, 8192)?;
            let slice = llm::parse_json_object_slice(&reply);
            let plan: LlmIngestPlanV1 = serde_json::from_str(slice)
                .map_err(|e| format!("ingest-llm JSON parse error: {e}; raw={reply}"))?;
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }
            let sc = parse_scope(&scope);
            let sid = eng.ingest_raw(uri.clone(), &body, sc.clone(), "cli");
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let body_short = truncate_chars(&body, 16000);
                let vec = llm::embed_first(&app, &body_short)?;
                repo.upsert_embedding(&format!("source:{}", sid.0), &vec)?;
            }
            for c in &plan.claims {
                let tier = parse_memory_tier(&c.tier).unwrap_or(MemoryTier::Semantic);
                let cid = eng.file_claim(c.text.clone(), sc.clone(), tier, "cli");
                eng.attach_sources(cid, &[sid])?;
                eng.save_to_repo(&repo)?;
                eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
                if cli.vectors {
                    let app = llm::load_app_config(&cli.llm_config)?;
                    let vec = llm::embed_first(&app, &c.text)?;
                    repo.upsert_embedding(&format_claim_doc_id(cid), &vec)?;
                }
            }
            for ed in &plan.entities {
                let kind = EntityKind::parse(&ed.kind);
                let entity = Entity {
                    id: EntityId(uuid::Uuid::new_v4()),
                    kind,
                    label: ed.label.clone(),
                    scope: sc.clone(),
                };
                let _ = eng.add_entity(entity);
            }
            for rd in &plan.relationships {
                let from_id = eng
                    .store
                    .entities
                    .values()
                    .find(|e| e.label.eq_ignore_ascii_case(&rd.from_label))
                    .map(|e| e.id);
                let to_id = eng
                    .store
                    .entities
                    .values()
                    .find(|e| e.label.eq_ignore_ascii_case(&rd.to_label))
                    .map(|e| e.id);
                if let (Some(from), Some(to)) = (from_id, to_id) {
                    let rel = RelationKind::parse(&rd.relation);
                    let edge = TypedEdge {
                        from,
                        to,
                        relation: rel,
                        confidence: 0.7,
                        source_ids: vec![sid],
                    };
                    let _ = eng.add_edge(edge);
                }
            }
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            // summary 页固定为 vault 约定的 Summary 类型 + 五段正文（与 batch-ingest 对齐）
            if plan.should_materialize_summary_page() {
                let title = if plan.summary_title.trim().is_empty() {
                    "ingest-summary".to_string()
                } else {
                    plan.summary_title.trim().to_string()
                };
                let md = plan.to_five_section_summary_body(Some(&uri));
                let status = initial_status_for(Some(&EntryType::Summary), &schema);
                let page = WikiPage::new(title, md, sc.clone())
                    .with_entry_type(EntryType::Summary)
                    .with_status(status);
                eng.store.pages.insert(page.id, page);
                eng.save_to_repo(&repo)?;
                eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("ingested source={}", sid.0);
        }
        Cmd::Ingest { uri, body, scope } => {
            let sid = eng.ingest_raw(uri, &body, parse_scope(&scope), "cli");
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let body_short = truncate_chars(&body, 16000);
                let vec = llm::embed_first(&app, &body_short)?;
                repo.upsert_embedding(&format!("source:{}", sid.0), &vec)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("ingested source={}", sid.0);
        }
        Cmd::FileClaim { text, scope, tier } => {
            let tier = parse_tier(&tier)?;
            let cid = eng.file_claim(text, parse_scope(&scope), tier, "cli");
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let t = eng.store.claims[&cid].text.clone();
                let vec = llm::embed_first(&app, &t)?;
                repo.upsert_embedding(&format_claim_doc_id(cid), &vec)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("claim_id={}", cid.0);
        }
        Cmd::SupersedeClaim {
            old_claim_id,
            new_text,
            scope,
            tier,
        } => {
            let old = wiki_core::ClaimId(uuid::Uuid::parse_str(&old_claim_id)?);
            let tier = parse_tier(&tier)?;
            let new_id = eng.supersede(old, new_text, parse_scope(&scope), tier, "cli")?;
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let t = eng.store.claims[&new_id].text.clone();
                let vec = llm::embed_first(&app, &t)?;
                repo.upsert_embedding(&format_claim_doc_id(new_id), &vec)?;
            }
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("new_claim_id={}", new_id.0);
        }
        Cmd::Query {
            query,
            rrf_k,
            per_stream_limit,
            write_page,
            page_title,
            entry_type,
        } => {
            let ctx = QueryContext::new(&query)
                .with_rrf_k(rrf_k)
                .with_per_stream_limit(per_stream_limit)
                .with_viewer_scope(viewer.clone());
            let vec_override = if cli.vectors {
                let app = llm::load_app_config(&cli.llm_config)?;
                let qv = llm::embed_first(&app, &query)?;
                let raw = repo.search_embeddings_cosine(&qv, per_stream_limit.saturating_mul(8))?;
                let ids: Vec<String> = raw
                    .into_iter()
                    .filter(|(id, _)| doc_id_visible_to_viewer(id, &eng.store, &viewer))
                    .map(|(id, _)| id)
                    .take(per_stream_limit)
                    .collect();
                if ids.is_empty() {
                    None
                } else {
                    Some(ids)
                }
            } else {
                None
            };
            let graph_override = if let Some(ref path) = cli.graph_extras_file {
                let extras = read_graph_extras_lines(path)?;
                let ports = InMemorySearchPorts::new(&eng.store, Some(viewer.clone()));
                let kernel = SearchPorts::graph_ranked_ids(&ports, &query, per_stream_limit);
                Some(merge_graph_rankings(kernel, extras, per_stream_limit))
            } else {
                None
            };
            let ranked = eng.query_pipeline_memory(
                &ctx,
                OffsetDateTime::now_utc(),
                "cli",
                vec_override,
                graph_override,
            );
            if write_page {
                let title = page_title.unwrap_or_else(|| format!("query-{}", timestamp_slug()));
                let page = query_to_page(
                    &title,
                    &query,
                    &ranked,
                    viewer.clone(),
                    parse_entry_type_opt(&entry_type)?,
                    &schema,
                );
                eng.store.pages.insert(page.id, page);
            }
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            for (id, score) in ranked.into_iter().take(20) {
                println!("{score:.6}\t{id}");
            }
        }
        Cmd::Lint => {
            run_lint_job(&mut eng, &repo, &viewer, sync_wiki, wiki_root.as_deref())?;
        }
        Cmd::Gap {
            low_coverage_threshold,
            write_page,
        } => {
            run_gap_job(
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                low_coverage_threshold,
                write_page,
                &schema,
            )?;
        }
        Cmd::Promote { claim_id } => {
            let cid = wiki_core::ClaimId(uuid::Uuid::parse_str(&claim_id)?);
            eng.promote_if_qualified(cid, "cli", &viewer)?;
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("promoted {claim_id}");
        }
        Cmd::PromotePage { page_id, to, force } => {
            let pid = wiki_core::PageId(uuid::Uuid::parse_str(&page_id)?);
            // 解析目标状态：未指定时按 rule 自动取下一跳
            let to_status = match to {
                Some(s) => wiki_core::EntryStatus::parse(&s)
                    .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?,
                None => {
                    // 查找当前 page 的 entry_type → rule → 找 from == page.status 的第一条 promotion
                    let page = eng.store.pages.get(&pid).ok_or("page not found")?;
                    let et = page.entry_type.as_ref().ok_or("page has no entry_type")?;
                    let rule = eng
                        .schema
                        .find_lifecycle_rule(et)
                        .ok_or("no lifecycle rule")?;
                    let promo = rule
                        .promotions
                        .iter()
                        .find(|p| p.from_status == page.status)
                        .ok_or("no next promotion available")?;
                    promo.to_status
                }
            };
            let now = OffsetDateTime::now_utc();
            eng.promote_page(pid, to_status, "cli", now, force)?;
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!("promoted page {page_id} to {to_status:?}");
        }
        Cmd::Crystallize {
            question,
            findings,
            files,
            lessons,
            entry_type,
        } => {
            let et = parse_entry_type_opt(&entry_type)?;
            let draft = eng.crystallize(
                SessionCrystallizationInput {
                    question,
                    findings,
                    files_touched: files,
                    lessons,
                    scope: Scope::Private {
                        agent_id: "cli".into(),
                    },
                },
                "cli",
            )?;
            // crystallize 内部已经 insert page，此处覆盖 entry_type 和 status
            let status = initial_status_for(et.as_ref(), &schema);
            if let Some(page) = eng.store.pages.get_mut(&draft.page.id) {
                if let Some(et) = et {
                    page.entry_type = Some(et);
                }
                if page.status != status {
                    page.status = status;
                    page.status_entered_at = Some(OffsetDateTime::now_utc());
                }
            }
            eng.save_to_repo(&repo)?;
            eng.flush_outbox_to_repo_with_policy(&repo, 128, 3)?;
            maybe_sync_projection(sync_wiki, wiki_root.as_deref(), &eng)?;
            println!(
                "page={} claims={}",
                draft.page.id.0,
                draft.claim_candidates.len()
            );
        }
        Cmd::ExportOutboxNdjson => {
            print!("{}", repo.export_outbox_ndjson()?);
        }
        Cmd::ExportOutboxNdjsonFrom { last_id } => {
            print!("{}", repo.export_outbox_ndjson_from_id(last_id)?);
        }
        Cmd::AckOutbox {
            up_to_id,
            consumer_tag,
        } => {
            let n = repo.mark_outbox_processed(up_to_id, &consumer_tag)?;
            println!("acked={n}");
        }
        Cmd::ConsumeToMempalace {
            last_id,
            consumer_tag,
        } => {
            let (dispatch, start_id, acked) = run_consume_to_mempalace_job(
                &eng,
                &repo,
                &consumer_tag,
                last_id,
                cli.palace.as_deref(),
            )?;
            println!(
                "seen={} dispatched={} ignored={} filtered={} unresolved={} start_id={start_id} acked={acked} consumer_tag={consumer_tag}",
                dispatch.lines_seen,
                dispatch.dispatched,
                dispatch.ignored,
                dispatch.filtered,
                dispatch.unresolved,
            );
        }
        Cmd::LlmSmoke { config, prompt } => {
            let cfg = llm::load_llm_config(&config)?;
            let out = llm::smoke_chat_completion(&cfg, &prompt)?;
            println!("{out}");
        }
        Cmd::Mcp { once } => {
            mcp::run_mcp(
                &cli.db,
                schema,
                &cli.viewer_scope,
                once,
                &cli.llm_config,
                cli.vectors,
                wiki_root.as_deref(),
                cli.palace
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .as_deref(),
            )?;
        }
        Cmd::Maintenance => {
            run_maintenance_job(&mut eng, &repo, &viewer, sync_wiki, wiki_root.as_deref())?;
        }
        Cmd::BatchIngest {
            ref vault,
            limit,
            dry_run,
            delay_secs,
        } => {
            let vault_dir = vault.clone().unwrap_or_else(default_vault_path);
            let heartbeat = AutomationHeartbeat {
                repo: &repo,
                run_id: None,
            };
            batch_ingest_cmd(
                &mut eng,
                &repo,
                &cli,
                &vault_dir,
                limit,
                dry_run,
                delay_secs,
                sync_wiki,
                wiki_root.as_deref(),
                &schema,
                &heartbeat,
            )?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: false },
        } => {
            run_daily_automation(
                &cli,
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                &schema,
            )?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::RunDaily { dry_run: true },
        } => {
            let jobs = automation_run_daily_jobs();
            let mut stdout = std::io::stdout().lock();
            run_automation_plan(&jobs, true, &mut stdout, |_| Ok(()))?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::Run { job },
        } => {
            let mut stdout = std::io::stdout().lock();
            run_single_automation_job(
                &mut stdout,
                job,
                &cli,
                &mut eng,
                &repo,
                &viewer,
                sync_wiki,
                wiki_root.as_deref(),
                &schema,
            )?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::LastFailures { limit },
        } => {
            let mut stdout = std::io::stdout().lock();
            print_automation_last_failures(&repo, limit, &mut stdout)?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::Status,
        } => {
            let jobs = automation_all_jobs();
            let mut stdout = std::io::stdout().lock();
            print_automation_status(&repo, &jobs, &mut stdout)?;
        }
        Cmd::Automation {
            cmd: AutomationCmd::Doctor { consumer_tag },
        } => {
            let jobs = automation_all_jobs();
            let mut stdout = std::io::stdout().lock();
            print_automation_doctor(&repo, &jobs, &consumer_tag, &mut stdout)?;
        }
        Cmd::Automation {
            cmd:
                AutomationCmd::Health {
                    consumer_tag,
                    summary_file,
                    exit_on_yellow,
                },
        } => {
            let report = collect_automation_health_report(
                &repo,
                &automation_all_jobs(),
                &consumer_tag,
                OffsetDateTime::now_utc(),
            )?;
            let rendered = render_automation_health_report(&report, &consumer_tag);
            print!("{rendered}");
            if let Some(path) = summary_file {
                std::fs::write(&path, &rendered)?;
                println!("summary_file={}", path.display());
            }
            emit_automation_health_alert(report.level);
            let should_exit = match report.level {
                AutomationHealthLevel::Red => true,
                AutomationHealthLevel::Yellow => exit_on_yellow,
                AutomationHealthLevel::Green => false,
            };
            if should_exit {
                std::process::exit(1);
            }
        }
        Cmd::Automation {
            cmd: AutomationCmd::VerifyRestore,
        } => {
            let wiki_root = wiki_root
                .as_deref()
                .ok_or_else(|| "--wiki-dir 是 automation verify-restore 的必填参数".to_string())?;
            let report = collect_restore_verify_report(
                &cli.db,
                &repo,
                wiki_root,
                cli.palace.as_deref(),
                DEFAULT_MEMPALACE_CONSUMER_TAG,
            )?;
            print!(
                "{}",
                render_restore_verify_report(&report, DEFAULT_MEMPALACE_CONSUMER_TAG)
            );
        }
        Cmd::Automation {
            cmd: AutomationCmd::ListJobs,
        } => unreachable!(),
        // SchemaValidate 已在 main() 中短路，此处不可达
        Cmd::SchemaValidate { .. } => unreachable!(),
    }
    Ok(())
}

pub(crate) fn doc_id_visible_to_viewer(
    doc_id: &str,
    store: &InMemoryStore,
    viewer: &Scope,
) -> bool {
    if let Some(rest) = doc_id.strip_prefix("claim:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .claims
                .get(&ClaimId(u))
                .map(|c| document_visible_to_viewer(&c.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("page:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .pages
                .get(&PageId(u))
                .map(|p| document_visible_to_viewer(&p.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("entity:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .entities
                .get(&EntityId(u))
                .map(|e| document_visible_to_viewer(&e.scope, viewer))
                .unwrap_or(false);
        }
        return false;
    }
    if let Some(rest) = doc_id.strip_prefix("source:") {
        if let Ok(u) = uuid::Uuid::parse_str(rest) {
            return store
                .sources
                .get(&SourceId(u))
                .map(|s| document_visible_to_viewer(&s.scope, viewer))
                .unwrap_or(false);
        }
    }
    false
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_storage::AutomationJobFailureSummary;

    fn sample_record(
        status: AutomationRunStatus,
        heartbeat_at: OffsetDateTime,
    ) -> AutomationRunRecord {
        AutomationRunRecord {
            id: 1,
            job_name: "lint".into(),
            started_at: heartbeat_at - Duration::minutes(5),
            finished_at: None,
            status,
            duration_ms: None,
            error_summary: None,
            heartbeat_at,
        }
    }

    #[test]
    fn effective_entry_type_defaults_to_concept() {
        assert_eq!(effective_ingest_entry_type(None), EntryType::Concept);
    }

    #[test]
    fn effective_entry_type_preserves_explicit() {
        assert_eq!(
            effective_ingest_entry_type(Some(EntryType::Entity)),
            EntryType::Entity
        );
        assert_eq!(
            effective_ingest_entry_type(Some(EntryType::Synthesis)),
            EntryType::Synthesis
        );
    }

    #[test]
    fn automation_run_daily_plan_is_fixed_and_ordered() {
        let jobs = automation_run_daily_jobs();
        let labels: Vec<&str> = jobs.iter().copied().map(automation_job_name).collect();
        assert_eq!(
            labels,
            vec![
                "batch-ingest",
                "lint",
                "maintenance",
                "consume-to-mempalace",
            ]
        );
    }

    #[test]
    fn automation_job_registry_lists_named_jobs_in_stable_order() {
        let labels: Vec<&str> = automation_job_specs()
            .iter()
            .map(|spec| automation_job_name(spec.job))
            .collect();
        assert_eq!(
            labels,
            vec![
                "batch-ingest",
                "lint",
                "maintenance",
                "consume-to-mempalace",
                "llm-smoke",
            ]
        );
        assert!(automation_job_spec(AutomationJob::LlmSmoke).requires_network);
        assert!(!automation_job_spec(AutomationJob::LlmSmoke).in_daily);
    }

    #[test]
    fn automation_run_daily_dry_run_prints_plan_only() {
        let jobs = automation_run_daily_jobs();
        let mut out = Vec::new();
        let mut called = Vec::new();

        run_automation_plan(&jobs, true, &mut out, |job| {
            called.push(job);
            Ok(())
        })
        .unwrap();

        assert!(called.is_empty());
        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("automation run-daily plan:"));
        assert!(stdout.contains("1. batch-ingest"));
        assert!(stdout.contains("2. lint"));
        assert!(stdout.contains("3. maintenance"));
        assert!(stdout.contains("4. consume-to-mempalace"));
        assert!(stdout.contains("dry-run: no jobs executed"));
    }

    #[test]
    fn automation_run_daily_stops_after_first_failure() {
        let jobs = automation_run_daily_jobs();
        let mut out = Vec::new();
        let mut seen = Vec::new();

        let err = run_automation_plan(&jobs, false, &mut out, |job| {
            seen.push(job);
            if job == AutomationJob::Lint {
                Err("boom".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();

        assert_eq!(seen, vec![AutomationJob::BatchIngest, AutomationJob::Lint]);
        assert!(err.to_string().contains("boom"));
        let stdout = String::from_utf8(out).unwrap();
        assert!(stdout.contains("automation: running batch-ingest"));
        assert!(stdout.contains("automation: finished batch-ingest"));
        assert!(stdout.contains("automation: running lint"));
        assert!(!stdout.contains("automation: finished lint"));
        assert!(!stdout.contains("automation: running consume-to-mempalace"));
        assert!(!stdout.contains("automation: finished consume-to-mempalace"));
    }

    #[test]
    fn consume_start_id_prefers_progress_and_respects_last_id_floor() {
        let progress = OutboxConsumerProgress {
            consumer_tag: "mempalace".into(),
            acked_up_to_id: Some(3),
            acked_at: None,
            backlog_events: 2,
        };
        assert_eq!(effective_consume_start_id(&progress, 0), 3);
        assert_eq!(effective_consume_start_id(&progress, 5), 5);

        let empty_progress = OutboxConsumerProgress {
            consumer_tag: "mempalace".into(),
            acked_up_to_id: None,
            acked_at: None,
            backlog_events: 0,
        };
        assert_eq!(effective_consume_start_id(&empty_progress, 0), 0);
        assert_eq!(effective_consume_start_id(&empty_progress, 4), 4);
    }

    #[test]
    fn stale_heartbeat_thresholds_classify_expected_levels() {
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        let thresholds = automation_health_thresholds();
        let green = sample_record(AutomationRunStatus::Running, now - Duration::hours(1));
        let yellow = sample_record(AutomationRunStatus::Running, now - Duration::hours(8));
        let red = sample_record(AutomationRunStatus::Running, now - Duration::hours(36));
        let finished = sample_record(AutomationRunStatus::Succeeded, now - Duration::hours(36));

        assert_eq!(
            classify_stale_heartbeat(&green, now, thresholds),
            AutomationHealthLevel::Green
        );
        assert_eq!(
            classify_stale_heartbeat(&yellow, now, thresholds),
            AutomationHealthLevel::Yellow
        );
        assert_eq!(
            classify_stale_heartbeat(&red, now, thresholds),
            AutomationHealthLevel::Red
        );
        assert_eq!(
            classify_stale_heartbeat(&finished, now, thresholds),
            AutomationHealthLevel::Green
        );
    }

    #[test]
    fn consecutive_failure_thresholds_classify_expected_levels() {
        let thresholds = automation_health_thresholds();
        assert_eq!(
            classify_consecutive_failures(0, thresholds),
            AutomationHealthLevel::Green
        );
        assert_eq!(
            classify_consecutive_failures(2, thresholds),
            AutomationHealthLevel::Yellow
        );
        assert_eq!(
            classify_consecutive_failures(3, thresholds),
            AutomationHealthLevel::Red
        );
    }

    #[test]
    fn backlog_thresholds_classify_expected_levels() {
        let thresholds = automation_health_thresholds();
        assert_eq!(
            classify_backlog(0, thresholds),
            AutomationHealthLevel::Green
        );
        assert_eq!(
            classify_backlog(25, thresholds),
            AutomationHealthLevel::Yellow
        );
        assert_eq!(
            classify_backlog(120, thresholds),
            AutomationHealthLevel::Red
        );
    }

    #[test]
    fn health_report_render_includes_manual_action_and_failures() {
        let report = AutomationHealthReport {
            level: AutomationHealthLevel::Red,
            issues: vec![AutomationHealthIssue {
                level: AutomationHealthLevel::Red,
                target: "lint".into(),
                code: "consecutive-failures",
                detail: "consecutive_failures=3".into(),
            }],
            outbox: OutboxStats {
                head_id: 10,
                total_events: 10,
                unprocessed_events: 4,
            },
            progress: OutboxConsumerProgress {
                consumer_tag: "mempalace".into(),
                acked_up_to_id: Some(6),
                acked_at: None,
                backlog_events: 4,
            },
            failures: vec![AutomationJobFailureSummary {
                job_name: "lint".into(),
                consecutive_failures: 3,
                latest_failure: Some(sample_record(
                    AutomationRunStatus::Failed,
                    OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
                )),
            }],
        };

        let rendered = render_automation_health_report(&report, "mempalace");
        assert!(rendered.contains("automation health: status=red"));
        assert!(rendered.contains("code=consecutive-failures"));
        assert!(rendered.contains("job=lint consecutive_failures=3"));
        assert!(rendered.contains("manual_action=investigate_and_fix_before_next_daily_run"));
    }

    #[test]
    fn env_vars_override_health_thresholds() {
        // Use unique env-var values that differ from every default so the test
        // is unambiguous even if run in parallel with other tests.
        std::env::set_var("WIKI_HEALTH_BACKLOG_YELLOW", "7");
        std::env::set_var("WIKI_HEALTH_BACKLOG_RED", "14");
        std::env::set_var("WIKI_HEALTH_FAIL_YELLOW", "5");
        std::env::set_var("WIKI_HEALTH_FAIL_RED", "10");
        std::env::set_var("WIKI_HEALTH_STALE_YELLOW_HOURS", "3");
        std::env::set_var("WIKI_HEALTH_STALE_RED_HOURS", "9");

        let t = automation_health_thresholds();
        assert_eq!(t.backlog_yellow, 7);
        assert_eq!(t.backlog_red, 14);
        assert_eq!(t.consecutive_failures_yellow, 5);
        assert_eq!(t.consecutive_failures_red, 10);
        assert_eq!(t.stale_heartbeat_yellow, Duration::hours(3));
        assert_eq!(t.stale_heartbeat_red, Duration::hours(9));

        // Restore defaults so other tests in the same process are not affected.
        for key in &[
            "WIKI_HEALTH_BACKLOG_YELLOW",
            "WIKI_HEALTH_BACKLOG_RED",
            "WIKI_HEALTH_FAIL_YELLOW",
            "WIKI_HEALTH_FAIL_RED",
            "WIKI_HEALTH_STALE_YELLOW_HOURS",
            "WIKI_HEALTH_STALE_RED_HOURS",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn gap_empty_db_no_panic() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };
        let findings = eng.run_gap_scan(Some(&viewer), 2);
        assert!(findings.is_empty(), "空库不应该有 gap");
    }

    #[test]
    fn gap_reports_findings() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook).unwrap();
        let viewer = Scope::Private {
            agent_id: "cli".into(),
        };

        // 添加一条 claim，但没有 page 引用它，会触发 gap.missing_xref
        eng.file_claim(
            "项目使用 Redis 进行缓存",
            Scope::Private {
                agent_id: "cli".into(),
            },
            MemoryTier::Semantic,
            "test",
        );

        let findings = eng.run_gap_scan(Some(&viewer), 2);
        assert!(!findings.is_empty(), "应该检测到 gap");
        assert!(
            findings.iter().any(|f| f.code == "gap.missing_xref"),
            "应该检测到 missing_xref"
        );

        // 测试 markdown 报告输出
        let md = gap_report_markdown(&findings);
        assert!(md.contains("# Gap Report"));
        assert!(md.contains("gap.missing_xref"));

        // 测试写入报告文件
        let wiki_dir = dir.path().join("wiki");
        std::fs::create_dir_all(&wiki_dir).unwrap();
        let path = write_gap_report(&wiki_dir, "gap-test", &findings).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("gap.missing_xref"));
    }
}

fn maybe_sync_projection(
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    eng: &LlmWikiEngine<NoopWikiHook>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !sync_wiki {
        return Ok(());
    }
    if let Some(root) = wiki_root {
        let stats = write_projection(root, &eng.store, &eng.audits)?;
        println!(
            "projection pages={} claims={} sources={}",
            stats.pages_written, stats.claims_written, stats.sources_written
        );
    }
    Ok(())
}

fn run_lint_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let findings = eng.run_basic_lint("cli", Some(viewer));
    eng.save_to_repo(repo)?;
    eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;
    if let Some(root) = wiki_root {
        let report = write_lint_report(root, &format!("lint-{}", timestamp_slug()), &findings)?;
        println!("lint_report={}", report.display());
    }
    maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    for f in findings {
        println!("{:?}\t{}\t{}", f.severity, f.code, f.message);
    }
    Ok(())
}

/// 将 GapFinding 列表渲染为 markdown 字符串。
///
/// 共享函数：`write_gap_report`（写文件）和 `gap_report_markdown`（写 page）都调用它。
fn render_gap_markdown(findings: &[GapFinding]) -> String {
    let severity_order = [GapSeverity::High, GapSeverity::Medium, GapSeverity::Low];
    let mut grouped: std::collections::BTreeMap<&str, Vec<&GapFinding>> =
        std::collections::BTreeMap::new();
    for f in findings {
        let key = match f.severity {
            GapSeverity::High => "high",
            GapSeverity::Medium => "medium",
            GapSeverity::Low => "low",
        };
        grouped.entry(key).or_default().push(f);
    }
    let mut md = String::from("# Gap Report\n\n");
    md.push_str(&format!("- total gaps: `{}`\n\n", findings.len()));
    for sev in &severity_order {
        let key = match sev {
            GapSeverity::High => "high",
            GapSeverity::Medium => "medium",
            GapSeverity::Low => "low",
        };
        if let Some(items) = grouped.get(key) {
            md.push_str(&format!("## {key}\n\n"));
            for item in items {
                let subject_info = match (&item.subject, &item.subject_label) {
                    (Some(s), Some(l)) => format!(" (subject={s}, label={l})"),
                    (Some(s), None) => format!(" (subject={s})"),
                    (None, Some(l)) => format!(" (label={l})"),
                    (None, None) => String::new(),
                };
                md.push_str(&format!(
                    "- `{}` {}{}\n",
                    item.code, item.message, subject_info
                ));
            }
            md.push('\n');
        }
    }
    md
}

/// 生成 gap 报告的 markdown 文件，写入 wiki/reports/gap-{timestamp}.md
fn write_gap_report(
    wiki_root: &std::path::Path,
    report_name: &str,
    findings: &[GapFinding],
) -> std::io::Result<std::path::PathBuf> {
    use std::fs;

    let reports_dir = wiki_root.join("reports");
    fs::create_dir_all(&reports_dir)?;
    let filename = if report_name.ends_with(".md") {
        report_name.to_string()
    } else {
        format!("{report_name}.md")
    };
    let out = reports_dir.join(filename);
    let md = render_gap_markdown(findings);
    fs::write(&out, md)?;
    Ok(out)
}

fn run_gap_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    low_coverage_threshold: usize,
    write_page: bool,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let findings = eng.run_gap_scan(Some(viewer), low_coverage_threshold);
    eng.save_to_repo(repo)?;
    eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;

    let report_md = gap_report_markdown(&findings);

    if let Some(root) = wiki_root {
        let report_path = write_gap_report(root, &format!("gap-{}", timestamp_slug()), &findings)?;
        println!("gap_report={}", report_path.display());
    }

    if write_page {
        let title = format!("gap-report-{}", timestamp_slug());
        let status = initial_status_for(Some(&EntryType::LintReport), schema);
        let page = WikiPage::new(title, report_md, viewer.clone())
            .with_entry_type(EntryType::LintReport)
            .with_status(status);
        eng.store.pages.insert(page.id, page);
        eng.save_to_repo(repo)?;
        eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;
    }

    maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    for f in &findings {
        println!("{:?}\t{}\t{}", f.severity, f.code, f.message);
    }
    Ok(())
}
/// 将 GapFinding 列表渲染为 markdown 字符串（用于 --write-page）
fn gap_report_markdown(findings: &[GapFinding]) -> String {
    render_gap_markdown(findings)
}

fn run_maintenance_job(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = OffsetDateTime::now_utc();
    eng.apply_confidence_decay_all(now, 30.0);
    let findings = eng.run_basic_lint("cli", Some(viewer));
    let mut promoted = 0u32;
    let claim_ids: Vec<ClaimId> = eng.store.claims.keys().copied().collect();
    for cid in claim_ids {
        if eng.promote_if_qualified(cid, "cli", viewer).is_ok() {
            promoted += 1;
        }
    }
    let pages_marked = eng.mark_stale_pages(now);
    let pages_cleaned = eng.cleanup_expired_pages(now);
    eng.save_to_repo(repo)?;
    eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;
    maybe_sync_projection(sync_wiki, wiki_root, eng)?;
    println!(
        "decay=applied lint_findings={} promoted={promoted} pages_marked_needs_update={pages_marked} pages_auto_cleaned={pages_cleaned}",
        findings.len()
    );
    Ok(())
}

fn effective_consume_start_id(progress: &OutboxConsumerProgress, requested_last_id: i64) -> i64 {
    progress
        .acked_up_to_id
        .map_or(requested_last_id, |acked| acked.max(requested_last_id))
}

fn run_consume_to_mempalace_job(
    eng: &LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    consumer_tag: &str,
    last_id: i64,
    palace_path: Option<&std::path::Path>,
) -> Result<(OutboxDispatchStats, i64, usize), Box<dyn std::error::Error>> {
    let progress = repo.get_outbox_consumer_progress(consumer_tag)?;
    let start_id = effective_consume_start_id(&progress, last_id);
    let stats = repo.get_outbox_stats()?;
    if start_id >= stats.head_id {
        return Ok((OutboxDispatchStats::default(), start_id, 0));
    }

    let ndjson = repo.export_outbox_ndjson_from_id(start_id)?;
    if ndjson.is_empty() {
        return Ok((OutboxDispatchStats::default(), start_id, 0));
    }

    let resolver = EngineResolver { store: &eng.store };
    let dispatch = if let Some(pp) = palace_path {
        let live = LiveMempalaceSink::open(pp, "wiki")?;
        consume_outbox_ndjson_with_resolver_and_stats(&live, &resolver, &ndjson)?
    } else {
        consume_outbox_ndjson_with_resolver_and_stats(&CliMempalaceSink, &resolver, &ndjson)?
    };
    let acked = repo.mark_outbox_processed(stats.head_id, consumer_tag)?;
    Ok((dispatch, start_id, acked))
}

fn dispatch_automation_job(
    job: AutomationJob,
    heartbeat: &AutomationHeartbeat<'_>,
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    heartbeat.tick();
    match job {
        AutomationJob::BatchIngest => {
            let vault = cli.wiki_dir.clone().unwrap_or_else(default_vault_path);
            batch_ingest_cmd(
                eng, repo, cli, &vault, None, false, 1, sync_wiki, wiki_root, schema, heartbeat,
            )
        }
        AutomationJob::Lint => run_lint_job(eng, repo, viewer, sync_wiki, wiki_root),
        AutomationJob::Maintenance => run_maintenance_job(eng, repo, viewer, sync_wiki, wiki_root),
        AutomationJob::ConsumeToMempalace => run_consume_to_mempalace_job(
            eng,
            repo,
            DEFAULT_MEMPALACE_CONSUMER_TAG,
            0,
            cli.palace.as_deref(),
        )
        .map(|_| ()),
        AutomationJob::LlmSmoke => {
            let cfg = llm::load_llm_config(&cli.llm_config)?;
            let out = llm::smoke_chat_completion(&cfg, "Say 'ok' only.")?;
            println!("{out}");
            Ok(())
        }
    }
}

fn run_single_automation_job<W: Write>(
    out: &mut W,
    job: AutomationJob,
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = automation_job_spec(job);
    writeln!(
        out,
        "automation: running {} requires_network={} daily={}",
        automation_job_name(spec.job),
        if spec.requires_network { "yes" } else { "no" },
        if spec.in_daily { "yes" } else { "no" }
    )?;
    run_automation_job(repo, job, |hb| {
        dispatch_automation_job(
            job, hb, cli, eng, repo, viewer, sync_wiki, wiki_root, schema,
        )
    })?;
    let latest = latest_automation_run_or_error(repo, job)?;
    writeln!(
        out,
        "automation: finished {} {}",
        automation_job_name(job),
        format_automation_record(&latest)
    )?;
    Ok(())
}

fn run_daily_automation(
    cli: &Cli,
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    viewer: &Scope,
    sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let jobs = automation_run_daily_jobs();
    let mut stdout = std::io::stdout().lock();
    run_automation_plan(&jobs, false, &mut stdout, |job| {
        run_automation_job(repo, job, |hb| {
            dispatch_automation_job(
                job, hb, cli, eng, repo, viewer, sync_wiki, wiki_root, schema,
            )
        })
    })
}

fn query_to_page(
    title: &str,
    query: &str,
    ranked: &[(String, f64)],
    scope: Scope,
    entry_type: Option<EntryType>,
    schema: &DomainSchema,
) -> WikiPage {
    let mut md = format!("# {title}\n\n## Query\n\n{query}\n\n## Top Results\n\n");
    for (doc, score) in ranked.iter().take(20) {
        md.push_str(&format!("- `{doc}` score={score:.6}\n"));
    }
    let status = initial_status_for(entry_type.as_ref(), schema);
    let page = WikiPage::new(title, md, scope).with_status(status);
    match entry_type {
        Some(et) => page.with_entry_type(et),
        None => page,
    }
}

fn read_graph_extras_lines(path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let s = std::fs::read_to_string(path)?;
    Ok(s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect())
}

fn timestamp_slug() -> String {
    let now = OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "now".to_string())
        .replace(':', "-")
}

/// 用当前 in-memory store 反解 `ClaimUpserted` / `SourceIngested` 的 payload + scope。
struct EngineResolver<'a> {
    store: &'a InMemoryStore,
}

impl<'a> OutboxResolver for EngineResolver<'a> {
    fn claim(&self, id: wiki_core::ClaimId) -> Option<wiki_core::Claim> {
        self.store.claims.get(&id).cloned()
    }

    fn source_scope(&self, id: wiki_core::SourceId) -> Option<Scope> {
        self.store.sources.get(&id).map(|s| s.scope.clone())
    }
}

struct CliMempalaceSink;

impl MempalaceWikiSink for CliMempalaceSink {
    fn on_claim_upserted(&self, claim: &wiki_core::Claim) -> Result<(), MempalaceError> {
        // resolver 路径：打印 id + 文本前缀，证明 payload 已被还原；真正写入 palace 由
        // live sink 在 wiki-mempalace-bridge 的 `live` feature 下完成。
        let preview: String = claim.text.chars().take(80).collect();
        println!("mempalace claim_upserted {} {}", claim.id.0, preview);
        Ok(())
    }

    fn on_claim_event(&self, claim_id: wiki_core::ClaimId) -> Result<(), MempalaceError> {
        // 仅在 resolver 无法解析 claim 时走到这里（悬挂事件）。
        println!("mempalace claim_upserted(unresolved) {}", claim_id.0);
        Ok(())
    }

    fn on_claim_superseded(
        &self,
        old: wiki_core::ClaimId,
        new: wiki_core::ClaimId,
    ) -> Result<(), MempalaceError> {
        println!("mempalace claim_superseded {} -> {}", old.0, new.0);
        Ok(())
    }

    fn on_source_linked(
        &self,
        source_id: wiki_core::SourceId,
        claim_id: wiki_core::ClaimId,
    ) -> Result<(), MempalaceError> {
        println!("mempalace source_linked {} -> {}", source_id.0, claim_id.0);
        Ok(())
    }

    fn scope_filter(&self, _scope: &Scope) -> bool {
        true
    }

    fn on_source_ingested(&self, source_id: SourceId) -> Result<(), MempalaceError> {
        println!("mempalace source_ingested {}", source_id.0);
        Ok(())
    }
}

// ── batch-ingest 相关 ──

/// 一条 source 的扫描结果
struct SourceEntry {
    path: PathBuf,
    title: String,
    url: String,
    body: String,
    /// 来自 frontmatter `tags`（逗号分隔）
    source_tags: Vec<String>,
    created_at: String,
}

/// batch 单条写入 summary / 引擎时携带的 vault 元数据
struct BatchIngestContext {
    source_title: String,
    source_url: String,
}

/// 单条 source 编译结果
struct IngestOneStats {
    claims: usize,
    entities: usize,
    relationships: usize,
    source_id: String,
    /// 完整 LLM 计划（用于写 pages/summary 与调试）
    plan: LlmIngestPlanV1,
}

/// 解析 frontmatter 中的 tags 字符串（支持中英文逗号）
fn parse_tags_csv(raw: Option<&String>) -> Vec<String> {
    raw.map(|s| {
        s.split(|c: char| c == ',' || c == '，')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

/// YAML 双引号内转义（与 wiki-kernel 投影一致）
fn yaml_escape_vault(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 输出 `name:\n  - "..."` 或 `name: []`
fn yaml_string_list_block(name: &str, items: &[String]) -> String {
    if items.is_empty() {
        format!("{name}: []\n")
    } else {
        let mut out = format!("{name}:\n");
        for it in items {
            out.push_str(&format!("  - \"{}\"\n", yaml_escape_vault(it)));
        }
        out
    }
}

fn entry_status_yaml(status: EntryStatus) -> &'static str {
    match status {
        EntryStatus::Draft => "draft",
        EntryStatus::InReview => "in_review",
        EntryStatus::Approved => "approved",
        EntryStatus::NeedsUpdate => "needs_update",
    }
}

/// 扫描 vault/sources/ 中 compiled_to_wiki: false 的 source 文件
fn scan_uncompiled_sources(
    vault: &std::path::Path,
) -> Result<Vec<SourceEntry>, Box<dyn std::error::Error>> {
    let sources_dir = vault.join("sources");
    if !sources_dir.exists() {
        return Err(format!("sources 目录不存在：{}", sources_dir.display()).into());
    }
    let re_fm = regex::Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n")?;
    let mut entries = Vec::new();

    for dent in walkdir::WalkDir::new(&sources_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = dent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        let fm_caps = re_fm.captures(&content);
        let fm_text = fm_caps
            .as_ref()
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("");
        let fm = parse_frontmatter_kv(fm_text);

        if fm.get("compiled_to_wiki").map(|v| v.as_str()) != Some("false") {
            continue;
        }

        let title = fm.get("title").cloned().unwrap_or_default();
        let url = fm.get("url").cloned().unwrap_or_default();
        let source_tags = parse_tags_csv(fm.get("tags"));
        let created_at = fm.get("created_at").cloned().unwrap_or_default();

        let body = if let Some(caps) = fm_caps {
            let fm_end = caps.get(0).unwrap().end();
            content[fm_end..].trim().to_string()
        } else {
            content.trim().to_string()
        };

        if body.len() < 50 {
            eprintln!("  跳过（正文过短）：{}", title);
            continue;
        }

        entries.push(SourceEntry {
            path: path.to_path_buf(),
            title,
            url,
            body,
            source_tags,
            created_at,
        });
    }

    entries.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(entries)
}

/// 简易 YAML frontmatter key: value 解析
fn parse_frontmatter_kv(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once(':') {
            let key = key.trim().to_string();
            let val = val.trim().to_string();
            let val = val
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(&val)
                .to_string();
            if !key.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

/// 编译单条 source：LLM 抽取 + 写入引擎
fn ingest_one_source(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    cfg: &llm::LlmConfig,
    uri: &str,
    body: &str,
    scope: &Scope,
    vectors: bool,
    llm_config_path: &std::path::Path,
    schema: &DomainSchema,
    batch: &BatchIngestContext,
) -> Result<IngestOneStats, Box<dyn std::error::Error>> {
    let user = format!("Source URI:\n{uri}\n\nBody:\n{body}");
    let reply = llm::complete_chat(cfg, llm::ingest_llm_system_prompt(), &user, 8192)?;
    let slice = llm::parse_json_object_slice(&reply);
    let plan: LlmIngestPlanV1 =
        serde_json::from_str(slice).map_err(|e| format!("JSON parse error: {e}; raw={reply}"))?;

    let sid = eng.ingest_raw(uri, body, scope.clone(), "batch-ingest");
    eng.save_to_repo(repo)?;
    eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;

    if vectors {
        let app = llm::load_app_config(llm_config_path)?;
        let body_short = truncate_chars(body, 16000);
        let vec = llm::embed_first(&app, &body_short)?;
        repo.upsert_embedding(&format!("source:{}", sid.0), &vec)?;
    }

    for c in &plan.claims {
        let tier = parse_memory_tier(&c.tier).unwrap_or(MemoryTier::Semantic);
        let cid = eng.file_claim(c.text.clone(), scope.clone(), tier, "batch-ingest");
        eng.attach_sources(cid, &[sid])?;
        eng.save_to_repo(repo)?;
        eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;
        if vectors {
            let app = llm::load_app_config(llm_config_path)?;
            let vec = llm::embed_first(&app, &c.text)?;
            repo.upsert_embedding(&format_claim_doc_id(cid), &vec)?;
        }
    }

    for ed in &plan.entities {
        let kind = EntityKind::parse(&ed.kind);
        let entity = Entity {
            id: EntityId(uuid::Uuid::new_v4()),
            kind,
            label: ed.label.clone(),
            scope: scope.clone(),
        };
        // schema 可能拒绝不在白名单的 kind，跳过即可
        let _ = eng.add_entity(entity);
    }

    for rd in &plan.relationships {
        let from_id = eng
            .store
            .entities
            .values()
            .find(|e| e.label.eq_ignore_ascii_case(&rd.from_label))
            .map(|e| e.id);
        let to_id = eng
            .store
            .entities
            .values()
            .find(|e| e.label.eq_ignore_ascii_case(&rd.to_label))
            .map(|e| e.id);
        if let (Some(from), Some(to)) = (from_id, to_id) {
            let rel = RelationKind::parse(&rd.relation);
            let edge = TypedEdge {
                from,
                to,
                relation: rel,
                confidence: 0.7,
                source_ids: vec![sid],
            };
            // schema 可能拒绝不在白名单的 relation，跳过即可
            let _ = eng.add_edge(edge);
        }
    }
    eng.save_to_repo(repo)?;
    eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;

    // summary 页：磁盘与引擎均约定为 EntryType::Summary + 五段正文
    if plan.should_materialize_summary_page() {
        let page_title = format!("摘要：{}", batch.source_title);
        let foot_url = if batch.source_url.trim().is_empty() {
            uri
        } else {
            batch.source_url.as_str()
        };
        let md = plan.to_five_section_summary_body(Some(foot_url));
        let et = EntryType::Summary;
        let status = initial_status_for(Some(&et), schema);
        let page = WikiPage::new(page_title, md, scope.clone())
            .with_entry_type(et)
            .with_status(status);
        eng.store.pages.insert(page.id, page);
        eng.save_to_repo(repo)?;
        eng.flush_outbox_to_repo_with_policy(repo, 128, 3)?;
    }

    Ok(IngestOneStats {
        claims: plan.claims.len(),
        entities: plan.entities.len(),
        relationships: plan.relationships.len(),
        source_id: sid.0.to_string(),
        plan: plan.clone(),
    })
}

/// 以 Notion / vault-standards 完整契约写 summary 页到 `pages/summary/`
fn write_batch_summary(
    wiki_root: &std::path::Path,
    source_title: &str,
    plan: &LlmIngestPlanV1,
    source_url: &str,
    source_tags: &[String],
    source_created_at: &str,
    schema: &DomainSchema,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary_dir = wiki_root.join("pages").join("summary");
    std::fs::create_dir_all(&summary_dir)?;

    // 文件名：中文标题，仅将 `/` 替换为 `-`（与 docs/vault-standards.md 一致）
    let filename = format!("摘要：{}.md", source_title.replace('/', "-"));
    let path = summary_dir.join(&filename);

    let now = time::OffsetDateTime::now_utc();
    let now_str = now.format(&time::format_description::well_known::Rfc3339)?;
    let created_str = if source_created_at.trim().is_empty() {
        now_str.clone()
    } else {
        source_created_at.trim().to_string()
    };

    let status = initial_status_for(Some(&EntryType::Summary), schema);
    let status_s = entry_status_yaml(status);
    let conf = plan.normalized_summary_confidence();
    let foot = if source_url.trim().is_empty() {
        None
    } else {
        Some(source_url.trim())
    };
    let body_sections = plan.to_five_section_summary_body(foot);

    let title_esc = yaml_escape_vault(&format!("摘要：{source_title}"));
    let url_esc = yaml_escape_vault(source_url);

    let mut fm = String::from("---\n");
    fm.push_str(&format!("title: \"{title_esc}\"\n"));
    fm.push_str("entry_type: summary\n");
    fm.push_str(&format!("status: {status_s}\n"));
    fm.push_str(&format!("confidence: {conf}\n"));
    fm.push_str(&format!("source_url: \"{url_esc}\"\n"));
    fm.push_str(&yaml_string_list_block("source_tags", source_tags));
    fm.push_str(&yaml_string_list_block("tags", &plan.tags));
    fm.push_str(&format!(
        "created_at: \"{}\"\n",
        yaml_escape_vault(&created_str)
    ));
    fm.push_str(&format!(
        "updated_at: \"{}\"\n",
        yaml_escape_vault(&now_str)
    ));
    fm.push_str(&format!(
        "last_compiled_at: \"{}\"\n",
        yaml_escape_vault(&now_str)
    ));
    fm.push_str("compiled_by: batch-ingest\n");
    fm.push_str("---\n\n");

    let h1_esc = source_title.replace('/', "-");
    let content = format!(
        "{fm}# 摘要：{h1_esc}\n\n{body_sections}",
        fm = fm,
        h1_esc = h1_esc,
        body_sections = body_sections
    );

    std::fs::write(&path, content)?;
    Ok(())
}

/// batch-ingest 子命令入口
fn default_vault_path() -> PathBuf {
    if let Ok(v) = std::env::var("WIKI_VAULT_DIR") {
        return PathBuf::from(v);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Documents").join("wiki")
}

fn batch_ingest_cmd(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    repo: &SqliteRepository,
    cli: &Cli,
    vault: &std::path::Path,
    limit: Option<usize>,
    dry_run: bool,
    delay_secs: u64,
    _sync_wiki: bool,
    wiki_root: Option<&std::path::Path>,
    schema: &DomainSchema,
    heartbeat: &AutomationHeartbeat<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    heartbeat.tick();
    eprintln!("扫描未编译 source...");
    let mut sources = scan_uncompiled_sources(vault)?;
    eprintln!("  → 找到 {} 条未编译 source", sources.len());

    if let Some(n) = limit {
        sources.truncate(n);
        eprintln!("  → --limit {}，处理前 {} 条", n, sources.len());
    }

    if dry_run {
        for (i, s) in sources.iter().enumerate() {
            println!(
                "{}/{}) {} ({} 字符)",
                i + 1,
                sources.len(),
                s.title,
                s.body.len()
            );
        }
        return Ok(());
    }

    if sources.is_empty() {
        eprintln!("  → nothing to compile, done.");
        return Ok(());
    }

    let cfg = llm::load_llm_config(&cli.llm_config)?;
    let scope = parse_scope("private:batch-ingest");

    let mut ok_count = 0usize;
    let mut err_count = 0usize;

    for (i, src) in sources.iter().enumerate() {
        heartbeat.tick();
        let uri = if src.url.is_empty() {
            format!("file://{}", src.path.display())
        } else {
            src.url.clone()
        };

        eprintln!("[{}/{}] {}...", i + 1, sources.len(), src.title);

        let batch_ctx = BatchIngestContext {
            source_title: src.title.clone(),
            source_url: src.url.clone(),
        };

        match ingest_one_source(
            eng,
            repo,
            &cfg,
            &uri,
            &src.body,
            &scope,
            cli.vectors,
            &cli.llm_config,
            schema,
            &batch_ctx,
        ) {
            Ok(stats) => {
                eprintln!(
                    "  ✓ claims={} entities={} rels={} source={}",
                    stats.claims, stats.entities, stats.relationships, stats.source_id,
                );

                // 更新 source .md 的 compiled_to_wiki 标记
                let content = std::fs::read_to_string(&src.path)?;
                let new_content =
                    content.replace("compiled_to_wiki: false", "compiled_to_wiki: true");
                if new_content != content {
                    std::fs::write(&src.path, new_content)?;
                }

                // 按 vault-standards 写 summary 页到 pages/summary/
                if wiki_root.is_some() && stats.plan.should_materialize_summary_page() {
                    write_batch_summary(
                        wiki_root.unwrap(),
                        &src.title,
                        &stats.plan,
                        &src.url,
                        &src.source_tags,
                        &src.created_at,
                        schema,
                    )?;
                }

                ok_count += 1;
            }
            Err(e) => {
                eprintln!("  ✗ 失败：{e}");
                err_count += 1;
            }
        }

        // 限流
        if i + 1 < sources.len() && delay_secs > 0 {
            std::thread::sleep(std::time::Duration::from_secs(delay_secs));
        }
    }

    eprintln!("\n完成：成功={ok_count} 失败={err_count}");
    Ok(())
}
