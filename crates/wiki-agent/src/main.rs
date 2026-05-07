mod answer;
mod chat;
mod config;
mod doctor;
mod events;
mod evidence;
mod harness;
mod llm_adapter;
mod manager;
mod mcp_fallback;
mod memory;
mod planner;
mod render_cli;
mod roles;
mod session_store;
mod slash;
mod task;
mod tool_backend;
mod tui;
mod web_tool;
mod worker;
mod worker_tools;

use clap::{Parser, Subcommand};
use config::AgentConfig;
use planner::WebMode;
use roles::WorkerRole;
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
        #[arg(long, value_enum, default_value_t = WebMode::Auto)]
        web: WebMode,
        #[arg(long, value_delimiter = ',')]
        web_providers: Vec<String>,
        #[arg(long, default_value_t = false)]
        allow_private_web_search: bool,
        #[arg(long)]
        session: Option<String>,
        #[arg(long, default_value_t = false)]
        tui: bool,
        #[arg(long, hide = true)]
        fake_llm_response: Option<String>,
        #[arg(long, hide = true)]
        web_evidence_json: Option<PathBuf>,
    },
    /// Start the terminal UI chat.
    Tui {
        /// Optional startup prompt. If omitted, starts an interactive TUI.
        prompt: Vec<String>,
        #[arg(long, default_value = "agent_manager")]
        profile: String,
        #[arg(long, value_enum, default_value_t = ToolBackendKind::Native)]
        tool_backend: ToolBackendKind,
        #[arg(long, value_enum, default_value_t = WebMode::Auto)]
        web: WebMode,
        #[arg(long, value_delimiter = ',')]
        web_providers: Vec<String>,
        #[arg(long, default_value_t = false)]
        allow_private_web_search: bool,
        #[arg(long)]
        session: Option<String>,
        #[arg(long, hide = true)]
        fake_llm_response: Option<String>,
        #[arg(long, hide = true)]
        web_evidence_json: Option<PathBuf>,
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
    /// Run a manager-worker task.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Inspect or curate durable agent memory.
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    List,
    Show { session_id: String },
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    Run {
        /// Goal for the manager. If omitted, uses a generic inspect goal.
        prompt: Vec<String>,
        /// Optional role override. If omitted, the manager routes from prompt.
        #[arg(long, value_enum)]
        task: Option<WorkerRole>,
        #[arg(long, value_enum, default_value_t = ToolBackendKind::Native)]
        tool_backend: ToolBackendKind,
        #[arg(long, value_enum, default_value_t = WebMode::Auto)]
        web: WebMode,
        #[arg(long, value_delimiter = ',')]
        web_providers: Vec<String>,
        #[arg(long, default_value_t = false)]
        allow_private_web_search: bool,
        /// Allow fixer apply behind the writer lease.
        #[arg(long, default_value_t = false)]
        apply: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Optional wiki-cli binary path for mcp-child fallback calls.
        #[arg(long)]
        wiki_cli: Option<PathBuf>,
        #[arg(long, hide = true)]
        web_evidence_json: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum MemoryCommand {
    Curate {
        #[arg(long)]
        session: Option<String>,
        #[arg(long, default_value_t = false)]
        apply: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Status {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Search {
        query: Vec<String>,
        #[arg(long, default_value_t = false)]
        json: bool,
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
        Command::Chat {
            prompt,
            profile,
            tool_backend,
            web,
            web_providers,
            allow_private_web_search,
            session,
            tui,
            fake_llm_response,
            web_evidence_json,
        } => {
            let input = if prompt.is_empty() {
                None
            } else {
                Some(prompt.join(" "))
            };
            let options = chat::ChatOptions {
                config,
                profile,
                tool_backend,
                web,
                web_providers,
                allow_private_web_search,
                session_id: session,
                one_shot_prompt: input,
                fake_llm_response,
                web_evidence_json,
            };
            if tui {
                tui::run(options)?;
            } else {
                chat::run(options)?;
            }
        }
        Command::Tui {
            prompt,
            profile,
            tool_backend,
            web,
            web_providers,
            allow_private_web_search,
            session,
            fake_llm_response,
            web_evidence_json,
        } => {
            let input = if prompt.is_empty() {
                None
            } else {
                Some(prompt.join(" "))
            };
            tui::run(chat::ChatOptions {
                config,
                profile,
                tool_backend,
                web,
                web_providers,
                allow_private_web_search,
                session_id: session,
                one_shot_prompt: input,
                fake_llm_response,
                web_evidence_json,
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
        Command::Agent { command } => match command {
            AgentCommand::Run {
                prompt,
                task,
                tool_backend,
                web,
                web_providers,
                allow_private_web_search,
                apply,
                json,
                wiki_cli,
                web_evidence_json,
            } => {
                let goal = if prompt.is_empty() {
                    "inspect wiki".to_string()
                } else {
                    prompt.join(" ")
                };
                let task = manager::Manager.plan(&goal, task, apply);
                let options = worker::WorkerRuntimeOptions {
                    backend: tool_backend,
                    web,
                    web_providers,
                    allow_private_web_search,
                    wiki_cli,
                    web_evidence_json,
                };
                let report = worker::WorkerRuntime::new(&config, options).execute(&task);
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print!("{}", report.render_text());
                }
            }
        },
        Command::Memory { command } => match command {
            MemoryCommand::Curate {
                session,
                apply,
                json,
            } => {
                let store = session_store::SessionStore::open(config.session_db_path())?;
                let session_id = match session {
                    Some(id) => id,
                    None => {
                        store
                            .list_sessions()?
                            .into_iter()
                            .next()
                            .ok_or("no sessions available for memory curation")?
                            .id
                    }
                };
                let report = memory::curate_session(&config, &store, &session_id, apply)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print!("{}", report.render_text());
                }
            }
            MemoryCommand::Status { json } => {
                let report = memory::memory_status(&config)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print!("{}", report.render_text());
                }
            }
            MemoryCommand::Search { query, json } => {
                let goal = query.join(" ");
                let task = manager::Manager.plan(&goal, Some(WorkerRole::Search), false);
                let report = worker::WorkerRuntime::new(
                    &config,
                    worker::WorkerRuntimeOptions {
                        backend: ToolBackendKind::Native,
                        web: WebMode::Off,
                        web_providers: Vec::new(),
                        allow_private_web_search: false,
                        wiki_cli: None,
                        web_evidence_json: None,
                    },
                )
                .execute(&task);
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    print!("{}", report.render_text());
                }
            }
        },
    }
    Ok(())
}
