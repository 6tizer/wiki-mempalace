use crate::config::AgentConfig;
use std::path::Path;
use wiki_ai::{llm, web_search};

pub fn run_web_search(
    config: &AgentConfig,
    query: &str,
    providers: &[String],
    fake_path: Option<&Path>,
) -> Result<web_search::WebSearchRun, Box<dyn std::error::Error>> {
    if let Some(path) = fake_path {
        let text = std::fs::read_to_string(path)?;
        let run = serde_json::from_str(&text)?;
        return Ok(run);
    }
    let app = llm::load_app_config(&config.llm_config)?;
    web_search::run_search(&app, providers, query)
}
