use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "mempalace-rs",
    version,
    about = "Local-first palace memory CLI in Rust"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "~/.mempalace-rs")]
    pub palace: String,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Text)]
    pub output: OutputFormat,
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Copy, Clone, Debug, ValueEnum, Eq, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum MineMode {
    Projects,
    Convos,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Init {
        #[arg(long)]
        identity: Option<String>,
    },
    Mine {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = MineMode::Projects)]
        mode: MineMode,
        #[arg(long)]
        wing: Option<String>,
        #[arg(long)]
        hall: Option<String>,
        #[arg(long)]
        room: Option<String>,
        /// Memory bank / tenant id stored on each drawer (default: `default`).
        #[arg(long, default_value = "default")]
        bank: String,
    },
    Search {
        query: String,
        #[arg(long)]
        wing: Option<String>,
        #[arg(long)]
        hall: Option<String>,
        #[arg(long)]
        room: Option<String>,
        #[arg(long)]
        bank: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
    },
    Status,
    WakeUp {
        #[arg(long)]
        wing: Option<String>,
        #[arg(long)]
        bank: Option<String>,
    },
    Link {
        #[arg(long)]
        from_wing: String,
        #[arg(long)]
        from_room: String,
        #[arg(long)]
        to_wing: String,
        #[arg(long)]
        to_room: String,
    },
    Taxonomy {
        #[arg(long)]
        bank: Option<String>,
    },
    Traverse {
        #[arg(long)]
        wing: String,
        #[arg(long)]
        room: String,
        #[arg(long)]
        bank: Option<String>,
    },
    Split {
        path: PathBuf,
        #[arg(long, default_value = "### Session")]
        marker: String,
        #[arg(long, default_value_t = 20)]
        min_lines: usize,
        #[arg(long)]
        dry_run: bool,
    },
    Bench {
        #[arg(long, default_value_t = 50)]
        samples: usize,
        #[arg(long, default_value_t = 5)]
        top_k: usize,
        #[arg(long, value_enum, default_value_t = BenchMode::Random)]
        mode: BenchMode,
        #[arg(long)]
        report: Option<PathBuf>,
    },
    Banner,
    Principles,
    KgAdd {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        predicate: String,
        #[arg(long)]
        object: String,
        #[arg(long)]
        valid_from: Option<String>,
        #[arg(long)]
        source_drawer_id: Option<i64>,
    },
    KgQuery {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        as_of: Option<String>,
    },
    KgTimeline {
        #[arg(long)]
        subject: String,
    },
    KgStats,
    KgConflicts,
    KgInvalidate {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        predicate: String,
        #[arg(long)]
        object: String,
        #[arg(long)]
        ended: Option<String>,
    },
    /// Synthesize an answer from retrieved drawers (requires `llm` in config).
    Reflect {
        query: String,
        #[arg(long, default_value_t = 8)]
        search_limit: usize,
        #[arg(long)]
        bank: Option<String>,
    },
    /// Extract SPO triples via LLM into `kg_facts` (requires `llm` in config).
    Extract {
        #[arg(long)]
        text: Option<String>,
        #[arg(long)]
        drawer_id: Option<i64>,
    },
    Mcp {
        #[arg(long)]
        once: bool,
        #[arg(long, value_enum, default_value_t = McpTransport::Stdio)]
        transport: McpTransport,
        #[arg(long)]
        quiet: bool,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum BenchMode {
    Random,
    Fixed,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum McpTransport {
    Stdio,
}
