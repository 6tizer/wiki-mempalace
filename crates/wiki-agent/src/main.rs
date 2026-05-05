mod config;
mod doctor;
mod mcp_fallback;
mod tool_backend;

use clap::{Parser, Subcommand};
use config::AgentConfig;
use std::path::PathBuf;
use tool_backend::ToolBackendKind;

#[derive(Debug, Parser)]
#[command(name = "wiki-agent")]
#[command(about = "Interactive agent entrypoint for wiki-mempalace.", long_about = None)]
struct Cli {
    #[arg(long, default_value = "wiki.db")]
    db: PathBuf,
    #[arg(long)]
    wiki_dir: Option<PathBuf>,
    #[arg(long, default_value = "private:cli")]
    viewer_scope: String,
    #[arg(long, default_value = "llm-config.toml")]
    llm_config: PathBuf,
    #[arg(long, default_value_t = false)]
    vectors: bool,
    #[arg(long)]
    palace: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect agent runtime and tool backend availability.
    Doctor {
        #[arg(long, value_enum, default_value_t = ToolBackendKind::Native)]
        tool_backend: ToolBackendKind,
        /// Optional wiki-cli binary path for mcp-child fallback discovery.
        #[arg(long)]
        wiki_cli: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = AgentConfig {
        db: cli.db,
        wiki_dir: cli.wiki_dir,
        viewer_scope: cli.viewer_scope,
        llm_config: cli.llm_config,
        vectors: cli.vectors,
        palace: cli.palace,
    };

    match cli.command {
        Command::Doctor {
            tool_backend,
            wiki_cli,
        } => {
            let report = doctor::run(&config, tool_backend, wiki_cli.as_deref())?;
            print!("{report}");
        }
    }
    Ok(())
}
