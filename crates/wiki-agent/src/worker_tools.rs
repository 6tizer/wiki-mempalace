use crate::config::AgentConfig;
use time::Duration;
use wiki_core::{DomainSchema, Scope};
use wiki_kernel::{LlmWikiEngine, NoopWikiHook};
use wiki_storage::{SqliteRepository, SqliteWriterLease};

pub struct EngineContext {
    pub repo: SqliteRepository,
    pub eng: LlmWikiEngine<NoopWikiHook>,
    pub schema: DomainSchema,
    pub viewer: Scope,
}

impl EngineContext {
    pub fn open(config: &AgentConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let repo = SqliteRepository::open(&config.db)?;
        let schema = DomainSchema::permissive_default();
        let eng = LlmWikiEngine::load_from_repo(schema.clone(), &repo, NoopWikiHook)?;
        let viewer = wiki_tools::parse_scope(&config.viewer_scope);
        Ok(Self {
            repo,
            eng,
            schema,
            viewer,
        })
    }
}

pub fn acquire_writer_lease(
    config: &AgentConfig,
    label: &str,
) -> Result<SqliteWriterLease, wiki_storage::StorageError> {
    SqliteWriterLease::acquire(
        &config.db,
        format!("wiki-agent:{label}"),
        Duration::minutes(10),
    )
}
