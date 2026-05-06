use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use wiki_core::StrategyExecutionActionKind;

use crate::automation::{automation_job_name, automation_job_needs_writer_lease, AutomationJob};
use crate::cli_utils::DEFAULT_MEMPALACE_CONSUMER_TAG;

#[derive(Parser)]
#[command(name = "wiki")]
#[command(
    about = "SQLite + Markdown wiki, RRF query, NDJSON outbox; optional embeddings & MemPalace hooks.",
    long_about = None
)]
pub struct Cli {
    #[arg(long, default_value = "wiki.db")]
    pub db: PathBuf,
    #[arg(long)]
    pub schema: Option<PathBuf>,
    #[arg(long)]
    pub wiki_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub sync_wiki: bool,
    /// 检索 / lint / promote 的视角 scope（多 agent 隔离）。例如 `private:cli` 或 `shared:team1`。
    #[arg(long, default_value = "private:cli")]
    pub viewer_scope: String,
    /// 使用 `llm-config.toml` 中 `[embed]` 做向量检索（需联网）。
    #[arg(long, default_value_t = false)]
    pub vectors: bool,
    #[arg(long, default_value = "llm-config.toml")]
    pub llm_config: PathBuf,
    /// 每行一个 `entity:` / `claim:` / `page:` doc id，与内核图路按轮次合并后作为 RRF 第三路。
    #[arg(long)]
    pub graph_extras_file: Option<PathBuf>,
    /// palace.db 路径（启用后 consume-to-mempalace 写入真实 palace 数据库）。
    #[arg(long)]
    pub palace: Option<PathBuf>,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExecutorAllow {
    FixAutoSafe,
}

impl ExecutorAllow {
    pub fn action_kind(self) -> StrategyExecutionActionKind {
        match self {
            ExecutorAllow::FixAutoSafe => StrategyExecutionActionKind::FixAutoSafe,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ExecutorAllow::FixAutoSafe => "fix_auto_safe",
        }
    }
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Inspect or smoke-test configured LLM profiles.
    AiProfile {
        #[command(subcommand)]
        cmd: AiProfileCmd,
    },
    /// Inspect or smoke-test configured web search providers.
    WebSearch {
        #[command(subcommand)]
        cmd: WebSearchCmd,
    },
    /// Run read-only governance scans for lifecycle, references, duplicates, and synthesis signals.
    Governance {
        #[command(subcommand)]
        cmd: GovernanceCmd,
    },
    /// Discover high-value synthesis candidates from internal wiki signals.
    ResearchSynthesis {
        #[command(subcommand)]
        cmd: ResearchSynthesisCmd,
    },
    Ingest {
        uri: String,
        body: String,
        #[arg(long, default_value = "private:cli")]
        scope: String,
        #[arg(long = "tag")]
        tags: Vec<String>,
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
        #[arg(long = "tag")]
        tags: Vec<String>,
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
        /// 可选：mempalace DB 路径（开启融合检索）
        #[arg(long)]
        palace_db: Option<String>,
        /// 可选：mempalace bank ID（配合 --palace-db 使用）
        #[arg(long, default_value = "wiki")]
        palace_bank: String,
    },
    /// 解释搜索结果。
    Explain {
        query: String,
        #[arg(long, default_value_t = 60.0)]
        rrf_k: f64,
        #[arg(long, default_value_t = 50)]
        per_stream_limit: usize,
        /// 可选：mempalace DB 路径（开启融合检索）
        #[arg(long)]
        palace_db: Option<String>,
        /// 可选：mempalace bank ID（配合 --palace-db 使用）
        #[arg(long, default_value = "wiki")]
        palace_bank: String,
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
    /// 检测并修复 lint/gap finding，输出修复动作列表。
    Fix {
        /// 只输出修复建议，不执行任何变更。
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// 只处理可自动修复的项（Auto 类型）。
        #[arg(long, default_value_t = false)]
        auto_only: bool,
        /// 执行自动修复（无此 flag 则仅输出列表）。
        #[arg(long, default_value_t = false)]
        write: bool,
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
    /// 生成问答式知识条目。
    Qa {
        /// 问题文本
        question: String,
        /// 回答文本
        answer: String,
        /// 可选：覆盖 EntryType（默认 qa）
        #[arg(long)]
        entry_type: Option<String>,
    },
    /// 聚合分析生成综合研究条目。
    Synthesis {
        /// 研究主题
        topic: String,
        /// 综合分析正文（省略则从 stdin 读取）
        #[arg(long)]
        body: Option<String>,
    },
    ExportOutboxNdjson,
    /// Verify row-level wiki_state rows against the legacy snapshot blob using a read-only DB handle.
    VerifyRowState {
        /// Emit machine-readable JSON instead of text lines.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    ExportOutboxNdjsonFrom {
        /// Consumer cursor to export from. Defaults to mempalace.
        #[arg(long, default_value = "mempalace")]
        consumer_tag: String,
        /// Legacy/manual start floor. The effective start is max(cursor, last_id).
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
    /// Read-only audit of an Obsidian vault before historical backfill.
    VaultAudit {
        /// Vault root directory.
        #[arg(long)]
        vault: PathBuf,
        /// Report directory. Must be inside <vault>/reports.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
    /// Plan/apply orphan governance from a timestamped vault audit.
    OrphanGovernance {
        #[command(subcommand)]
        command: OrphanGovernanceCmd,
    },
    /// Backfill historical vault sources/pages into wiki.db.
    VaultBackfill {
        /// Vault root directory.
        #[arg(long)]
        vault: PathBuf,
        /// Scope to assign to imported records.
        #[arg(long, default_value = "shared:wiki")]
        scope: String,
        /// Dry-run only. This is also the default when --apply is absent.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Apply frontmatter ID and DB/outbox changes.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Limit the number of vault records processed.
        #[arg(long)]
        limit: Option<usize>,
        /// Report directory. Defaults to <vault>/reports.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
    /// Initialize palace.db from wiki.db outbox.
    PalaceInit {
        /// Minimum outbox id; effective start also respects consumer progress.
        #[arg(long, default_value_t = 0)]
        last_id: i64,
        /// Consumer tag used for outbox ack / progress.
        #[arg(long, default_value = "mempalace")]
        consumer_tag: String,
        /// Report directory. Defaults to <wiki-dir>/reports or ./reports.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
    /// Read-only DB/Vault/Mempalace consistency audit.
    ConsistencyAudit,
    /// Build a validated DB/Vault/Mempalace consistency plan from an audit.
    ConsistencyPlan {
        /// Path to reports/consistency-audit-<timestamp>.json.
        #[arg(long)]
        audit_report: PathBuf,
        /// Report directory. Defaults to <wiki-dir>/reports from the audit.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
    /// Apply executable actions from a validated consistency plan. Defaults to dry-run.
    ConsistencyApply {
        /// Path to reports/consistency-plan-<timestamp>.json.
        #[arg(long)]
        plan: PathBuf,
        /// Mutate DB/Vault/Mempalace page mirror. Without this flag, dry-run only.
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// Collect read-only wiki metrics.
    Metrics {
        /// Consumer tag used for outbox ack / lag metrics.
        #[arg(long, default_value = DEFAULT_MEMPALACE_CONSUMER_TAG)]
        consumer_tag: String,
        /// Low coverage threshold used by gap scan.
        #[arg(long, default_value_t = 2)]
        low_coverage_threshold: usize,
        /// Print pretty JSON instead of text.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Also write a Markdown report to this path.
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Generate a read-only static operations dashboard.
    Dashboard {
        /// HTML output path.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Consumer tag used for outbox ack / lag metrics.
        #[arg(long, default_value = DEFAULT_MEMPALACE_CONSUMER_TAG)]
        consumer_tag: String,
        /// Low coverage threshold used by gap scan.
        #[arg(long, default_value_t = 2)]
        low_coverage_threshold: usize,
    },
    /// Produce read-only strategy suggestions.
    Suggest {
        /// Consumer tag used for outbox ack / lag metrics.
        #[arg(long, default_value = DEFAULT_MEMPALACE_CONSUMER_TAG)]
        consumer_tag: String,
        /// Low coverage threshold used by gap scan.
        #[arg(long, default_value_t = 2)]
        low_coverage_threshold: usize,
        /// Print pretty JSON instead of text.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Also derive an executor dry-run action plan from the suggestion report.
        #[arg(long, default_value_t = false)]
        executor_plan: bool,
        /// Also write timestamped JSON + Markdown reports to this directory.
        #[arg(long, num_args = 0..=1)]
        report_dir: Option<Option<PathBuf>>,
    },
    /// Apply an M12 executor plan with explicit allowlist guards.
    SuggestExecutorApply {
        /// JSON plan produced by `wiki-cli suggest --executor-plan --report-dir`.
        #[arg(long)]
        plan: PathBuf,
        /// Allow a typed action kind. Repeatable; only `fix-auto-safe` is supported now.
        #[arg(long = "allow", value_enum)]
        allow: Vec<ExecutorAllow>,
        /// Execute allowed actions. Without this flag the command only validates and previews.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Print pretty JSON instead of text.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Also write timestamped JSON + Markdown execution reports to this directory.
        #[arg(long, num_args = 0..=1)]
        report_dir: Option<Option<PathBuf>>,
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
        /// 可选：只处理 sources/<origin>/ 下的 source；all 表示不过滤
        #[arg(long)]
        origin: Option<String>,
        /// 可选：只处理指定 source Markdown 路径
        #[arg(long)]
        source_path: Option<PathBuf>,
        /// 编译写入 scope；默认 shared:wiki
        #[arg(long)]
        scope: Option<String>,
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
    /// Resolve production compiler deferred_resolutions with machine-only decisions.
    CompilerResolveDeferred {
        /// production-wiki-compiler JSON report path.
        #[arg(long)]
        report: PathBuf,
        /// Apply safe DB changes. Without this flag the command is dry-run.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Allow creating new canonical pages when the deferred item has no candidates.
        #[arg(long, default_value_t = false)]
        allow_create: bool,
        /// Report directory. Defaults to <wiki-dir>/reports when --wiki-dir is set.
        #[arg(long)]
        report_dir: Option<PathBuf>,
        /// Outbox consumer tag used when syncing Mempalace after apply.
        #[arg(long, default_value = DEFAULT_MEMPALACE_CONSUMER_TAG)]
        consumer_tag: String,
    },
    /// Run, inspect, and monitor scheduled automation jobs.
    Automation {
        #[command(subcommand)]
        cmd: AutomationCmd,
    },
    /// Incrementally sync Notion databases into wiki.db.
    NotionSync {
        /// Which DB to sync: x_bookmark | wechat | all
        #[arg(long, default_value = "all")]
        db_id: NotionDbTarget,
        /// Override incremental cursor start time (ISO 8601 UTC)
        #[arg(long)]
        since: Option<String>,
        /// Max pages to fetch per DB
        #[arg(long)]
        limit: Option<usize>,
        /// Print what would be synced without writing to DB
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Milliseconds between Notion API requests (minimum 100)
        #[arg(long, default_value_t = 350)]
        request_delay_ms: u64,
        /// Write back to Notion after sync (marks 已编译到Wiki checkbox)
        #[arg(long, default_value_t = false)]
        writeback_notion: bool,
        /// Re-fetch and update existing notion:// sources instead of skipping them.
        #[arg(long, default_value_t = false)]
        refresh_existing: bool,
        /// Tag policy for this sync run. Notion AI auto-fill is treated as a trusted source by default.
        #[arg(long, value_enum, default_value = "trusted-source")]
        tag_policy: NotionSyncTagPolicy,
        /// Print per-page processing details
        #[arg(long, default_value_t = false)]
        verbose: bool,
    },
    /// Backfill notion_page_index from historical vault source frontmatter.
    NotionSyncIndexBackfill {
        /// Vault root directory. Defaults to --wiki-dir when present.
        #[arg(long)]
        vault: Option<PathBuf>,
        /// Dry-run only. This is also the default when --apply is absent.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Write missing index rows.
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// Write DB-backed notion:// sources into vault sources/{origin}/ markdown.
    NotionSourceVaultSync {
        /// Vault root directory. Defaults to --wiki-dir when present.
        #[arg(long)]
        vault: Option<PathBuf>,
        /// Dry-run only. This is also the default when --apply is absent.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Write missing source markdown files.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Rewrite existing source frontmatter tags into Obsidian-safe tag names.
        #[arg(long, default_value_t = false)]
        repair_tags: bool,
        /// Rewrite existing DB-backed source markdown files when DB content changed.
        #[arg(long, default_value_t = false)]
        refresh_existing: bool,
    },
    /// Audit archived Notion pages and write a DB-first retirement plan. Dry-run only.
    NotionArchivedRetirement {
        #[command(subcommand)]
        command: NotionArchivedRetirementCmd,
    },
}

#[derive(Subcommand)]
pub enum AiProfileCmd {
    /// Run a minimal chat completion through a named LLM profile.
    Smoke {
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, default_value = "Say 'ok' only.")]
        prompt: String,
    },
}

#[derive(Subcommand)]
pub enum WebSearchCmd {
    /// Run one query through one or more configured web search providers.
    Smoke {
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        providers: Vec<String>,
        #[arg(long)]
        query: String,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum GovernanceCmd {
    /// Run a read-only unified governance scan.
    Scan {
        /// Print pretty JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Write sibling JSON + Markdown reports to this directory.
        #[arg(long)]
        report_dir: Option<PathBuf>,
        /// Low coverage threshold used by the embedded gap scan.
        #[arg(long, default_value_t = 2)]
        low_coverage_threshold: usize,
    },
    /// Build a typed dry-run fixer plan from a governance scan report.
    FixerPlan {
        /// Governance scan JSON report path.
        #[arg(long)]
        scan: PathBuf,
        /// Print pretty JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Write sibling JSON + Markdown reports to this directory.
        #[arg(long)]
        report_dir: Option<PathBuf>,
        /// Optional verified semantic patch proposal JSON.
        #[arg(long)]
        semantic_patches: Option<PathBuf>,
        /// Permit web search for actions that require external verification.
        #[arg(long, default_value_t = false)]
        allow_web_search: bool,
        /// Permit web search when scan viewer_scope is private.
        #[arg(long, default_value_t = false)]
        allow_private_web_search: bool,
        /// Force internal-only planning; web-required actions stay blocked.
        #[arg(long, default_value_t = false)]
        internal_only: bool,
        /// Maximum near-duplicate groups to verify by web search.
        #[arg(long, default_value_t = 5)]
        max_web_checks: usize,
    },
    /// Apply a typed evidence fixer plan. Defaults to preflight unless --apply is passed.
    FixerApply {
        /// Evidence fixer plan JSON path.
        #[arg(long)]
        plan: PathBuf,
        /// Apply policy. First supported policy is evidence-auto.
        #[arg(long, value_enum, default_value_t = EvidenceFixerPolicyArg::EvidenceAuto)]
        policy: EvidenceFixerPolicyArg,
        /// Execute mutations. Without this flag the command only preflights.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Print pretty JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Write sibling JSON + Markdown reports and tombstone files to this directory.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
    /// Restore one evidence fixer tombstone. Defaults to preflight unless --apply is passed.
    Restore {
        /// Evidence fixer tombstone JSON path.
        #[arg(long)]
        tombstone: PathBuf,
        /// Execute restore. Without this flag the command only preflights.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Print pretty JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Write sibling JSON + Markdown restore report to this directory.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
}

impl Cmd {
    pub fn writer_lease_label(&self) -> &'static str {
        match self {
            Cmd::Automation {
                cmd: AutomationCmd::RunDaily { .. },
            } => "automation-run-daily",
            Cmd::Automation {
                cmd: AutomationCmd::Run { job },
            } => automation_job_name(*job),
            Cmd::BatchIngest { .. } => "batch-ingest",
            Cmd::CompilerResolveDeferred { .. } => "compiler-resolve-deferred",
            Cmd::ConsistencyApply { .. } => "consistency-apply",
            Cmd::Mcp { .. } => "mcp",
            Cmd::NotionArchivedRetirement { .. } => "notion-archived-retirement",
            Cmd::NotionSync { .. } => "notion-sync",
            Cmd::NotionSyncIndexBackfill { .. } => "notion-sync-index-backfill",
            Cmd::SuggestExecutorApply { .. } => "suggest-executor-apply",
            Cmd::Governance {
                cmd: GovernanceCmd::FixerApply { .. },
            } => "governance-fixer-apply",
            Cmd::Governance {
                cmd: GovernanceCmd::Restore { .. },
            } => "governance-restore",
            Cmd::ResearchSynthesis {
                cmd: ResearchSynthesisCmd::Compose { .. },
            } => "research-synthesis-compose",
            Cmd::ResearchSynthesis {
                cmd: ResearchSynthesisCmd::Run { .. },
            } => "research-synthesis-run",
            Cmd::VaultBackfill { .. } => "vault-backfill",
            _ => "wiki-cli",
        }
    }

    pub fn needs_writer_lease(&self) -> bool {
        match self {
            Cmd::Ingest { .. }
            | Cmd::FileClaim { .. }
            | Cmd::SupersedeClaim { .. }
            | Cmd::Query { .. }
            | Cmd::Lint
            | Cmd::Gap { .. }
            | Cmd::Promote { .. }
            | Cmd::PromotePage { .. }
            | Cmd::Crystallize { .. }
            | Cmd::Qa { .. }
            | Cmd::Synthesis { .. }
            | Cmd::AckOutbox { .. }
            | Cmd::ConsumeToMempalace { .. }
            | Cmd::PalaceInit { .. }
            | Cmd::Maintenance
            | Cmd::Mcp { .. } => true,
            Cmd::IngestLlm { dry_run, .. } => !dry_run,
            Cmd::Fix { dry_run, write, .. } => *write && !dry_run,
            Cmd::SuggestExecutorApply { apply, .. } => *apply,
            Cmd::Governance {
                cmd: GovernanceCmd::FixerApply { apply, .. },
            } => *apply,
            Cmd::Governance {
                cmd: GovernanceCmd::Restore { apply, .. },
            } => *apply,
            Cmd::ResearchSynthesis {
                cmd: ResearchSynthesisCmd::Compose { apply, .. },
            } => *apply,
            Cmd::ResearchSynthesis {
                cmd: ResearchSynthesisCmd::Run { apply, .. },
            } => *apply,
            Cmd::VaultBackfill { apply, .. } => *apply,
            Cmd::ConsistencyApply { apply, .. } => *apply,
            Cmd::BatchIngest { dry_run, .. } => !dry_run,
            Cmd::CompilerResolveDeferred { apply, .. } => *apply,
            Cmd::Automation {
                cmd: AutomationCmd::RunDaily { dry_run },
            } => !dry_run,
            Cmd::Automation {
                cmd: AutomationCmd::Run { job },
            } => automation_job_needs_writer_lease(*job),
            Cmd::NotionSync { dry_run, .. } => !dry_run,
            Cmd::NotionSyncIndexBackfill { apply, .. } => *apply,
            Cmd::NotionSourceVaultSync { apply, .. } => *apply,
            Cmd::NotionArchivedRetirement {
                command: NotionArchivedRetirementCmd::Apply { apply, .. },
            } => *apply,
            Cmd::Automation { .. }
            | Cmd::NotionArchivedRetirement {
                command: NotionArchivedRetirementCmd::Plan { .. },
            }
            | Cmd::ExportOutboxNdjson
            | Cmd::VerifyRowState { .. }
            | Cmd::ExportOutboxNdjsonFrom { .. }
            | Cmd::Explain { .. }
            | Cmd::VaultAudit { .. }
            | Cmd::OrphanGovernance { .. }
            | Cmd::ConsistencyAudit
            | Cmd::ConsistencyPlan { .. }
            | Cmd::Metrics { .. }
            | Cmd::Dashboard { .. }
            | Cmd::Suggest { .. }
            | Cmd::Governance {
                cmd: GovernanceCmd::Scan { .. } | GovernanceCmd::FixerPlan { .. },
            }
            | Cmd::ResearchSynthesis {
                cmd: ResearchSynthesisCmd::Discover { .. },
            }
            | Cmd::AiProfile { .. }
            | Cmd::WebSearch { .. }
            | Cmd::LlmSmoke { .. }
            | Cmd::SchemaValidate { .. } => false,
        }
    }
}

#[derive(Subcommand)]
pub enum ResearchSynthesisCmd {
    /// Discover point/line/plane/volume synthesis candidates. Read-only.
    Discover {
        /// Optional governance scan JSON. If omitted, the command scans the current wiki first.
        #[arg(long)]
        scan: Option<PathBuf>,
        /// Print pretty JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Write sibling JSON + Markdown reports to this directory.
        #[arg(long)]
        report_dir: Option<PathBuf>,
        /// Maximum candidates from the combined single/double pool.
        #[arg(long, default_value_t = 1)]
        max_single_double: usize,
        /// Maximum triple-tag candidates.
        #[arg(long, default_value_t = 1)]
        max_triple: usize,
        /// Maximum quad-tag candidates.
        #[arg(long, default_value_t = 1)]
        max_quad: usize,
    },
    /// Compose one synthesis page from a discovery candidate.
    Compose {
        /// Candidate ID from a synthesis discovery report.
        #[arg(long)]
        candidate: String,
        /// Optional synthesis discovery JSON. If omitted, discovery runs first.
        #[arg(long)]
        discovery: Option<PathBuf>,
        /// Optional fake/precomputed web evidence JSON for tests or offline runs.
        #[arg(long)]
        web_evidence: Option<PathBuf>,
        /// Optional fake/precomputed draft JSON. If omitted, synthesis_writer is called.
        #[arg(long)]
        draft_json: Option<PathBuf>,
        /// Optional fake/precomputed verifier JSON. If omitted, synthesis_verifier is called.
        #[arg(long)]
        verifier_json: Option<PathBuf>,
        /// Do not send web search queries. Output is allowed to use internal evidence only.
        #[arg(long, default_value_t = false)]
        internal_only: bool,
        /// Permit external web search when active viewer scope is private.
        #[arg(long, default_value_t = false)]
        allow_private_web_search: bool,
        /// Write the synthesis page if verification passes.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Print pretty JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Write sibling JSON + Markdown reports to this directory.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
    /// Discover and compose the current top synthesis candidates.
    Run {
        /// Write synthesis pages if verification passes.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Do not send web search queries. Output is allowed to use internal evidence only.
        #[arg(long, default_value_t = false)]
        internal_only: bool,
        /// Permit external web search when active viewer scope is private.
        #[arg(long, default_value_t = false)]
        allow_private_web_search: bool,
        /// Print pretty JSON to stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Write sibling JSON + Markdown reports to this directory.
        #[arg(long)]
        report_dir: Option<PathBuf>,
        /// Maximum candidates from the combined single/double pool.
        #[arg(long, default_value_t = 1)]
        max_single_double: usize,
        /// Maximum triple-tag candidates.
        #[arg(long, default_value_t = 1)]
        max_triple: usize,
        /// Maximum quad-tag candidates.
        #[arg(long, default_value_t = 1)]
        max_quad: usize,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum EvidenceFixerPolicyArg {
    EvidenceAuto,
}

impl From<EvidenceFixerPolicyArg> for wiki_core::EvidenceFixerApplyPolicy {
    fn from(value: EvidenceFixerPolicyArg) -> Self {
        match value {
            EvidenceFixerPolicyArg::EvidenceAuto => {
                wiki_core::EvidenceFixerApplyPolicy::EvidenceAuto
            }
        }
    }
}

#[derive(Subcommand)]
pub enum OrphanGovernanceCmd {
    /// Ask LLM for a validated governance plan.
    Plan {
        /// Path to reports/vault-audit-<timestamp>.json.
        #[arg(long)]
        audit_report: PathBuf,
        /// Report directory. With --wiki-dir, must be under <wiki-dir>/reports.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
    /// Apply executable actions from a validated governance plan. Defaults to dry-run.
    Apply {
        /// Path to reports/orphan-governance-plan-<timestamp>.json.
        #[arg(long)]
        plan: PathBuf,
        /// Mutate vault files. Without this flag, dry-run only.
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
}

#[derive(Subcommand)]
pub enum AutomationCmd {
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
pub enum NotionDbTarget {
    #[value(name = "x_bookmark")]
    XBookmark,
    #[value(name = "wechat")]
    Wechat,
    #[value(name = "all")]
    All,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum NotionSyncTagPolicy {
    #[value(name = "strict")]
    Strict,
    #[value(name = "trusted-source")]
    TrustedSource,
    #[value(name = "bootstrap")]
    Bootstrap,
}

#[derive(Subcommand)]
pub enum NotionArchivedRetirementCmd {
    /// Pull Notion archived state and write a dry-run retirement plan.
    Plan {
        /// Report directory. Defaults to <wiki-dir>/reports when --wiki-dir is set.
        #[arg(long)]
        report_dir: Option<PathBuf>,
        /// Max indexed Notion pages to inspect.
        #[arg(long)]
        limit: Option<usize>,
        /// Milliseconds between Notion API requests (minimum 100)
        #[arg(long, default_value_t = 350)]
        request_delay_ms: u64,
    },
    /// Apply safe retirement actions from a generated plan. Defaults to dry-run.
    Apply {
        /// Path to notion-archived-retirement-plan-<timestamp>.json.
        #[arg(long)]
        plan: PathBuf,
        /// Mutate DB and matching Vault source files. Without this flag, dry-run only.
        #[arg(long, default_value_t = false)]
        apply: bool,
        /// Report directory. Defaults to <wiki-dir>/reports when --wiki-dir is set.
        #[arg(long)]
        report_dir: Option<PathBuf>,
    },
}
