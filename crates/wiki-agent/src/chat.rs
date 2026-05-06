use crate::answer;
use crate::config::AgentConfig;
use crate::evidence::{EvidencePack, InternalEvidence};
use crate::llm_adapter::{ChatModel, FakeChatModel, WikiAiChatModel};
use crate::memory;
use crate::planner::{self, WebMode};
use crate::render_cli;
use crate::session_store::SessionStore;
use crate::slash::{self, SlashCommand};
use crate::tool_backend::{discover_native_tools, ToolBackendKind};
use rusqlite::{params, Connection};
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use uuid::Uuid;
use wiki_core::{ClaimId, DomainSchema, PageId, SourceId};
use wiki_kernel::{LlmWikiEngine, NoopWikiHook};
use wiki_storage::SqliteRepository;
use wiki_tools::{ToolContext, ToolRegistry};

pub struct ChatOptions {
    pub config: AgentConfig,
    pub profile: String,
    pub tool_backend: ToolBackendKind,
    pub web: WebMode,
    pub web_providers: Vec<String>,
    pub allow_private_web_search: bool,
    pub session_id: Option<String>,
    pub one_shot_prompt: Option<String>,
    pub fake_llm_response: Option<String>,
    pub web_evidence_json: Option<PathBuf>,
}

pub(crate) struct ChatTurn {
    pub evidence: String,
    pub answer: String,
}

pub fn run(options: ChatOptions) -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = init_runtime(&options)?;

    if let Some(prompt) = options.one_shot_prompt.clone() {
        runtime.handle_user_message(&prompt, options.fake_llm_response.clone())?;
        runtime.curate_memory_on_close()?;
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
    runtime.curate_memory_on_close()?;
    Ok(())
}

pub(crate) fn init_runtime(
    options: &ChatOptions,
) -> Result<ChatRuntime, Box<dyn std::error::Error>> {
    let store = SessionStore::open(options.config.session_db_path())?;
    let title_hint = options.one_shot_prompt.as_deref().unwrap_or("chat session");
    let session_id =
        store.ensure_session(options.session_id.clone(), title_hint, &options.profile)?;
    Ok(ChatRuntime {
        config: options.config.clone(),
        profile: options.profile.clone(),
        tool_backend: options.tool_backend,
        web: options.web,
        web_providers: options.web_providers.clone(),
        allow_private_web_search: options.allow_private_web_search,
        web_evidence_json: options.web_evidence_json.clone(),
        session_id,
        store,
    })
}

pub(crate) struct ChatRuntime {
    config: AgentConfig,
    profile: String,
    tool_backend: ToolBackendKind,
    web: WebMode,
    web_providers: Vec<String>,
    allow_private_web_search: bool,
    web_evidence_json: Option<PathBuf>,
    session_id: String,
    store: SessionStore,
}

impl ChatRuntime {
    pub(crate) fn profile(&self) -> &str {
        &self.profile
    }

    fn handle_input_line(
        &mut self,
        input: &str,
        fake_response: Option<String>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some(command) = slash::parse(input) {
            return self.handle_slash(command);
        }
        let model = build_model(&self.config, &self.profile, fake_response)?;
        let turn = self.run_turn(input, model.as_ref())?;
        render_cli::render_system_line(&turn.evidence);
        render_cli::render_assistant_message(&turn.answer)?;
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
                if let Some(mode) = mode {
                    if let Ok(parsed) = mode.parse::<WebMode>() {
                        self.web = parsed;
                    }
                }
                render_cli::render_system_line(&format!("web={}", self.web));
            }
            SlashCommand::Memory => {
                render_cli::render_system_line("memory=session-store; durable memory enabled");
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
        fake_response: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let model = build_model(&self.config, &self.profile, fake_response)?;
        let turn = self.run_turn(prompt, model.as_ref())?;
        render_cli::render_system_line(&turn.evidence);
        render_cli::render_assistant_message(&turn.answer)?;
        Ok(())
    }

    pub(crate) fn run_prompt(
        &mut self,
        prompt: &str,
        fake_response: Option<String>,
    ) -> Result<ChatTurn, Box<dyn std::error::Error>> {
        let model = build_model(&self.config, &self.profile, fake_response)?;
        self.run_turn(prompt, model.as_ref())
    }

    fn run_turn(
        &mut self,
        prompt: &str,
        model: &dyn ChatModel,
    ) -> Result<ChatTurn, Box<dyn std::error::Error>> {
        self.store.add_message(&self.session_id, "user", prompt)?;
        let evidence = self.collect_evidence(prompt)?;
        let evidence_rendered = evidence.render();
        let system = system_prompt(&self.profile, self.tool_backend);
        let user_prompt = answer::build_user_prompt(prompt, &evidence);
        let answer = model.complete(&system, &user_prompt)?;
        self.store
            .add_message(&self.session_id, "assistant", &answer)?;
        Ok(ChatTurn {
            evidence: evidence_rendered,
            answer,
        })
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

    fn collect_evidence(&self, prompt: &str) -> Result<EvidencePack, Box<dyn std::error::Error>> {
        let internal = self.collect_internal_evidence(prompt)?;
        let mut pack = EvidencePack::new(internal);
        if !planner::should_use_web(prompt, self.web) {
            pack.web_status = Some("off".to_string());
            return Ok(pack);
        }
        if planner::private_scope_blocks_web(
            &self.config.viewer_scope,
            self.allow_private_web_search,
        ) {
            pack.web_status = Some("blocked: private scope".to_string());
            return Ok(pack);
        }
        let web = crate::web_tool::run_web_search(
            &self.config,
            prompt,
            &self.web_providers,
            self.web_evidence_json.as_deref(),
        )?;
        if !web.cross_verified {
            return Err(format!(
                "web search evidence is not cross-verified: providers={} domains={}",
                web.providers_succeeded.len(),
                web.distinct_domains
            )
            .into());
        }
        pack.web = Some(web);
        Ok(pack)
    }

    fn collect_internal_evidence(
        &self,
        prompt: &str,
    ) -> Result<Vec<InternalEvidence>, Box<dyn std::error::Error>> {
        let repo = SqliteRepository::open(&self.config.db)?;
        let schema = DomainSchema::permissive_default();
        let mut eng = LlmWikiEngine::load_from_repo(schema, &repo, NoopWikiHook)?;
        let viewer = wiki_tools::parse_scope(&self.config.viewer_scope);
        let palace = self
            .config
            .palace
            .as_ref()
            .map(|path| path.to_string_lossy().to_string());
        let registry = ToolRegistry;
        let value = registry.call(
            "wiki_query",
            json!({"query": prompt, "per_stream_limit": 5}),
            ToolContext {
                eng: &mut eng,
                repo: &repo,
                viewer: &viewer,
                llm_config_path: &self.config.llm_config,
                vectors: self.config.vectors,
                wiki_dir: None,
                palace_path: palace.as_deref(),
            },
        )?;
        let mut out: Vec<_> = value
            .get("results")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                Some(InternalEvidence {
                    doc_id: item.get("doc_id")?.as_str()?.to_string(),
                    score: item.get("score")?.as_f64().unwrap_or_default(),
                    title: None,
                    excerpt: None,
                })
            })
            .collect();
        let palace_conn = self
            .config
            .palace
            .as_ref()
            .and_then(|path| Connection::open(path).ok());
        for item in &mut out {
            enrich_internal_evidence(&eng, palace_conn.as_ref(), item);
        }
        Ok(out)
    }

    pub(crate) fn curate_memory_on_close(&self) -> Result<(), Box<dyn std::error::Error>> {
        let report = memory::curate_session(&self.config, &self.store, &self.session_id, true)?;
        if report.summary.total > 0 || !report.blockers.is_empty() {
            render_cli::render_system_line(&format!(
                "memory_curator total={} written={} rejected={} duplicates={}",
                report.summary.total,
                report.summary.written,
                report.summary.rejected,
                report.summary.duplicates
            ));
        }
        Ok(())
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

fn enrich_internal_evidence(
    eng: &LlmWikiEngine<NoopWikiHook>,
    palace_conn: Option<&Connection>,
    item: &mut InternalEvidence,
) {
    if let Some(uuid) = item.doc_id.strip_prefix("page:").and_then(parse_uuid) {
        if let Some(page) = eng.store.pages.get(&PageId(uuid)) {
            item.title = Some(page.title.clone());
            item.excerpt = Some(text_excerpt(&page.markdown, 900));
        }
        return;
    }
    if let Some(uuid) = item.doc_id.strip_prefix("claim:").and_then(parse_uuid) {
        if let Some(claim) = eng.store.claims.get(&ClaimId(uuid)) {
            item.title = Some("claim".to_string());
            item.excerpt = Some(text_excerpt(&claim.text, 700));
        }
        return;
    }
    if let Some(uuid) = item.doc_id.strip_prefix("source:").and_then(parse_uuid) {
        if let Some(source) = eng.store.sources.get(&SourceId(uuid)) {
            item.title = Some(source.uri.clone());
            item.excerpt = Some(text_excerpt(&source.body, 900));
        }
        return;
    }
    if let (Some(conn), Some(drawer_id)) = (
        palace_conn,
        item.doc_id
            .strip_prefix("mp_drawer:")
            .and_then(|value| value.parse::<i64>().ok()),
    ) {
        if let Ok((wing, hall, room, source_path, content)) = conn.query_row(
            "SELECT wing, hall, room, source_path, content FROM drawers WHERE id = ?1",
            params![drawer_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        ) {
            item.title = Some(
                first_markdown_heading(&content)
                    .unwrap_or_else(|| format!("{wing}/{hall}/{room} ({source_path})")),
            );
            item.excerpt = Some(text_excerpt(&content, 900));
        }
    }
}

fn parse_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value).ok()
}

fn first_markdown_heading(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn text_excerpt(text: &str, max_chars: usize) -> String {
    let trimmed = strip_frontmatter(text).trim();
    let mut out = String::new();
    let mut last_was_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
        if out.chars().count() >= max_chars {
            out.push_str("...");
            break;
        }
    }
    out.trim().to_string()
}

fn strip_frontmatter(text: &str) -> &str {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return text;
    }
    let mut offset = 4;
    for line in lines {
        offset += line.len() + 1;
        if line == "---" {
            return text.get(offset..).unwrap_or(text);
        }
    }
    text
}
