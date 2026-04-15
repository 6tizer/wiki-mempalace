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
    #[command(subcommand)]
    pub command: Commands,
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
    },
    Search {
        query: String,
        #[arg(long)]
        wing: Option<String>,
        #[arg(long)]
        hall: Option<String>,
        #[arg(long)]
        room: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
    },
    Status,
    WakeUp {
        #[arg(long)]
        wing: Option<String>,
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
    Taxonomy,
    Traverse {
        #[arg(long)]
        wing: String,
        #[arg(long)]
        room: String,
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
        #[arg(long)]
        report: Option<PathBuf>,
    },
}
