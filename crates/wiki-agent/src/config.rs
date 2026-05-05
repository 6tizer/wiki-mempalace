use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub db: PathBuf,
    pub wiki_dir: Option<PathBuf>,
    pub viewer_scope: String,
    pub llm_config: PathBuf,
    pub vectors: bool,
    pub palace: Option<PathBuf>,
}

impl AgentConfig {
    pub fn session_db_path(&self) -> PathBuf {
        if let Some(wiki_dir) = &self.wiki_dir {
            return wiki_dir.join(".wiki").join("wiki-agent.db");
        }
        self.db
            .parent()
            .map(|parent| parent.join("wiki-agent.db"))
            .unwrap_or_else(|| PathBuf::from("wiki-agent.db"))
    }
}
