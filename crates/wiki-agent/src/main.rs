mod chat;
mod config;
mod doctor;
mod events;
mod llm_adapter;
mod mcp_fallback;
mod render_cli;
mod session_store;
mod slash;
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
    /// Start pure CLI chat.
    Chat {
        /// Optional one-shot prompt. If omitted, starts an interactive REPL.
        prompt: Vec<String>,
        #[arg(long, default_value = "agent_manager")]
        profile: String,
        #[arg(long, value_enum, default_value_t = ToolBackendKind::Native)]
        tool_backend: ToolBackendKind,
        #[arg(long)]
        session: Option<String>,
        #[arg(long, default_value_t = false)]
        tui: bool,
        #[arg(long, hide = true)]
        fake_llm_response: Option<String>,
    },
    /// Inspect agent runtime and tool backend availability.
    Doctor {
        #[arg(long, value_enum, default_value_t = ToolBackendKind::Native)]
        tool_backend: ToolBackendKind,
        /// Optional wiki-cli binary path for mcp-child fallback discovery.
        #[arg(long)]
        wiki_cli: Option<PathBuf>,
    },
    /// Inspect saved chat sessions.
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    List,
    Show { session_id: String },
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
        Command::Chat {
            prompt,
            profile,
            tool_backend,
            session,
            tui,
            fake_llm_response,
        } => {
            if tui {
                eprintln!("wiki-agent tui is not implemented in PR3; falling back to CLI chat.");
            }
            let input = if prompt.is_empty() {
                None
            } else {
                Some(prompt.join(" "))
            };
            chat::run(chat::ChatOptions {
                config,
                profile,
                tool_backend,
                session_id: session,
                one_shot_prompt: input,
                fake_llm_response,
            })?;
        }
        Command::Doctor {
            tool_backend,
            wiki_cli,
        } => {
            let report = doctor::run(&config, tool_backend, wiki_cli.as_deref())?;
            print!("{report}");
        }
        Command::Session { command } => {
            let store = session_store::SessionStore::open(config.session_db_path())?;
            match command {
                SessionCommand::List => print!("{}", store.render_list()?),
                SessionCommand::Show { session_id } => {
                    print!("{}", store.render_session(&session_id)?)
                }
            }
        }
    }
    Ok(())
}
