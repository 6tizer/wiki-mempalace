use std::path::PathBuf;

use crate::llm;

pub(crate) fn run(config: PathBuf, prompt: String) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = llm::load_llm_config(&config)?;
    let out = llm::smoke_chat_completion(&cfg, &prompt)?;
    println!("{out}");
    Ok(())
}
