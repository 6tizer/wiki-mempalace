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
