use crate::config::AgentConfig;
use crate::llm_adapter::{ChatModel, FakeChatModel, WikiAiChatModel};
use crate::render_cli;
use crate::session_store::SessionStore;
use crate::slash::{self, SlashCommand};
use crate::tool_backend::{discover_native_tools, ToolBackendKind};
use std::io::{self, BufRead, Write};

pub struct ChatOptions {
    pub config: AgentConfig,
    pub profile: String,
    pub tool_backend: ToolBackendKind,
    pub session_id: Option<String>,
    pub one_shot_prompt: Option<String>,
    pub fake_llm_response: Option<String>,
}

pub fn run(options: ChatOptions) -> Result<(), Box<dyn std::error::Error>> {
    let store = SessionStore::open(options.config.session_db_path())?;
    let title_hint = options.one_shot_prompt.as_deref().unwrap_or("chat session");
    let session_id = store.ensure_session(options.session_id, title_hint, &options.profile)?;
    let mut runtime = ChatRuntime {
        config: options.config,
        profile: options.profile,
        tool_backend: options.tool_backend,
        session_id,
        store,
    };

    if let Some(prompt) = options.one_shot_prompt {
        let model = build_model(&runtime.config, &runtime.profile, options.fake_llm_response)?;
        runtime.handle_user_message(&prompt, model.as_ref())?;
        return Ok(());
    }

    render_cli::render_system_line(slash::help_text());
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut stdout = io::stdout();
    loop {
        write!(stdout, "> ")?;
        stdout.flush()?;
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if runtime.handle_input_line(line, options.fake_llm_response.clone())? {
            break;
        }
    }
    Ok(())
}

struct ChatRuntime {
    config: AgentConfig,
    profile: String,
    tool_backend: ToolBackendKind,
    session_id: String,
    store: SessionStore,
}

impl ChatRuntime {
    fn handle_input_line(
        &mut self,
        input: &str,
        fake_response: Option<String>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(command) = slash::parse(input) {
            return self.handle_slash(command);
        }
        let model = build_model(&self.config, &self.profile, fake_response)?;
        self.handle_user_message(input, model.as_ref())?;
        Ok(false)
    }

    fn handle_slash(&mut self, command: SlashCommand) -> Result<bool, Box<dyn std::error::Error>> {
        match command {
            SlashCommand::Help => render_cli::render_system_line(slash::help_text()),
            SlashCommand::Tools => self.render_tools(),
            SlashCommand::Profile(next) => {
                if let Some(next) = next {
                    self.profile = next;
                }
                render_cli::render_system_line(&format!("profile={}", self.profile));
            }
            SlashCommand::Web(mode) => {
                render_cli::render_system_line(&format!(
                    "web={}",
                    mode.as_deref().unwrap_or("auto")
                ));
            }
            SlashCommand::Memory => {
                render_cli::render_system_line("memory=session-store; durable memory lands in PR6");
            }
            SlashCommand::Sessions => print!("{}", self.store.render_list()?),
            SlashCommand::Exit => return Ok(true),
            SlashCommand::Unknown(command) => {
                render_cli::render_system_line(&format!("unknown command: {command}"));
            }
        }
        Ok(false)
    }

    fn handle_user_message(
        &mut self,
        prompt: &str,
        model: &dyn ChatModel,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.store.add_message(&self.session_id, "user", prompt)?;
        let system = system_prompt(&self.profile, self.tool_backend);
        let answer = model.complete(&system, prompt)?;
        self.store
            .add_message(&self.session_id, "assistant", &answer)?;
        render_cli::render_assistant_message(&answer)?;
        Ok(())
    }

    fn render_tools(&self) {
        let discovered = discover_native_tools();
        render_cli::render_system_line(&format!(
            "tools={} backend={}",
            discovered.tools.len(),
            discovered.backend
        ));
        for name in discovered.tools {
            render_cli::render_system_line(&format!("- {name}"));
        }
    }
}

fn build_model(
    config: &AgentConfig,
    profile: &str,
    fake_response: Option<String>,
) -> Result<Box<dyn ChatModel>, Box<dyn std::error::Error>> {
    if let Some(response) = fake_response {
        return Ok(Box::new(FakeChatModel::new(response)));
    }
    Ok(Box::new(WikiAiChatModel::load(
        &config.llm_config,
        profile,
    )?))
}

fn system_prompt(profile: &str, backend: ToolBackendKind) -> String {
    format!("You are wiki-agent. Answer directly. profile={profile}. tool_backend={backend:?}.")
}
