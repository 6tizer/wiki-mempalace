//! Optional OpenAI-compatible Chat Completions client (`POST {base}/chat/completions`).

use crate::service::LlmConfig;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

pub fn resolve_api_key(cfg: &LlmConfig) -> Option<String> {
    if let Some(ref env_name) = cfg.api_key_env {
        if !env_name.is_empty() {
            if let Ok(v) = std::env::var(env_name) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    cfg.api_key
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn llm_ready(cfg: &LlmConfig) -> bool {
    if !cfg.enabled {
        return false;
    }
    let Some(ref base) = cfg.base_url else {
        return false;
    };
    if base.trim().is_empty() {
        return false;
    }
    let Some(ref model) = cfg.model else {
        return false;
    };
    if model.trim().is_empty() {
        return false;
    }
    resolve_api_key(cfg).is_some()
}

pub fn chat_completion(cfg: &LlmConfig, messages: Vec<(&str, String)>) -> Result<String> {
    let key =
        resolve_api_key(cfg).context("missing LLM API key (set llm.api_key_env or llm.api_key)")?;
    let base = cfg
        .base_url
        .as_ref()
        .map(|s| s.trim_end_matches('/').to_string())
        .context("missing llm.base_url")?;
    let model = cfg
        .model
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .context("missing llm.model")?;
    let url = format!("{base}/chat/completions");
    let timeout = Duration::from_secs(cfg.timeout_secs.unwrap_or(120).max(5));
    let body = json!({
        "model": model,
        "messages": messages.iter().map(|(role, content)| json!({"role": role, "content": content})).collect::<Vec<_>>(),
        "temperature": 0.2
    });
    let r = ureq::post(&url)
        .set("Authorization", &format!("Bearer {key}"))
        .set("Content-Type", "application/json")
        .timeout(timeout)
        .send_json(body)
        .context("LLM HTTP request failed")?;
    let status = r.status();
    if status >= 400 {
        anyhow::bail!(
            "LLM HTTP {}: {}",
            status,
            r.into_string().unwrap_or_default()
        );
    }
    let resp: ChatCompletionResponse = r.into_json().context("LLM response was not valid JSON")?;
    let text = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();
    Ok(text.trim().to_string())
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Msg,
}

#[derive(Debug, Deserialize)]
struct Msg {
    content: Option<String>,
}

/// Strip optional ```json ... ``` fence from model output.
pub fn strip_code_fence(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest.trim_start_matches("json").trim_start();
        let rest = rest.trim_start_matches('\n');
        if let Some(i) = rest.rfind("```") {
            return rest[..i].trim().to_string();
        }
        return rest.trim().to_string();
    }
    t.to_string()
}

#[derive(Debug, Deserialize)]
pub struct ExtractedTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

pub fn parse_kg_triples_json(s: &str) -> Result<Vec<ExtractedTriple>> {
    let cleaned = strip_code_fence(s);
    let v: Vec<ExtractedTriple> = serde_json::from_str(&cleaned)
        .context("expected JSON array of {subject,predicate,object}")?;
    Ok(v)
}
