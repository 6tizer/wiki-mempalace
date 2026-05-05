use std::path::Path;

pub trait ChatModel {
    fn complete(&self, system: &str, user: &str) -> Result<String, Box<dyn std::error::Error>>;
}

pub struct WikiAiChatModel {
    config: wiki_ai::llm::LlmConfig,
}

impl WikiAiChatModel {
    pub fn load(llm_config_path: &Path, profile: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = wiki_ai::llm::load_llm_profile_config(llm_config_path, Some(profile))?;
        Ok(Self { config })
    }
}

impl ChatModel for WikiAiChatModel {
    fn complete(&self, system: &str, user: &str) -> Result<String, Box<dyn std::error::Error>> {
        wiki_ai::llm::complete_chat(
            &self.config,
            system,
            user,
            self.config.max_output_tokens.min(8192),
        )
    }
}

pub struct FakeChatModel {
    response: String,
}

impl FakeChatModel {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

impl ChatModel for FakeChatModel {
    fn complete(&self, _system: &str, _user: &str) -> Result<String, Box<dyn std::error::Error>> {
        Ok(self.response.clone())
    }
}
