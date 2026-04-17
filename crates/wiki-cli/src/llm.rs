use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfigFile {
    pub llm: LlmConfig,
    #[serde(default)]
    pub embed: Option<EmbedConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub max_retries: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EmbedConfig {
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub llm: LlmConfig,
    pub embed: Option<EmbedConfig>,
}

fn default_timeout_seconds() -> u64 {
    60
}

pub fn load_app_config(path: &Path) -> Result<AppConfig, Box<dyn std::error::Error>> {
    let s = std::fs::read_to_string(path)?;
    let parsed: LlmConfigFile = toml::from_str(&s)?;
    Ok(AppConfig {
        llm: parsed.llm,
        embed: parsed.embed,
    })
}

pub fn load_llm_config(path: &Path) -> Result<LlmConfig, Box<dyn std::error::Error>> {
    Ok(load_app_config(path)?.llm)
}

/// 从模型回复中截取最外层 `{ ... }` JSON 片段。
pub fn parse_json_object_slice(s: &str) -> &str {
    let t = s.trim();
    if let (Some(i), Some(j)) = (t.find('{'), t.rfind('}')) {
        if j >= i {
            return &t[i..=j];
        }
    }
    t
}

pub fn ingest_llm_system_prompt() -> &'static str {
    r#"You extract a structured plan from ONE source document for a knowledge wiki.
Reply with ONLY a single JSON object (no markdown fences), schema:
{
  "version": 1,
  "summary_title": "short title for a wiki page",
  "summary_markdown": "markdown body for an overview page",
  "claims": [ { "text": "atomic factual claim in the same language as the source", "tier": "semantic" } ],
  "entities": [ { "label": "EntityName", "kind": "library" } ],
  "relationships": [ { "from_label": "EntityA", "relation": "uses", "to_label": "EntityB" } ]
}
Rules:
- "tier" must be one of: working, episodic, semantic, procedural
- "kind" must be one of: person, project, library, concept, file_path, decision, other
- "relation" must be one of: uses, depends_on, contradicts, caused, fixed, supersedes, related
- claims: 0–12 items, each one short standalone sentence
- entities: 0–10 items, extract named entities (people, projects, libraries, concepts, decisions)
- relationships: 0–10 items, typed directed edges between entities
- summary_markdown may be empty if not useful
- Do not include keys other than those listed."#
}

/// 单次 chat completion，返回 assistant 文本（用于解析 JSON）。
pub fn complete_chat(
    cfg: &LlmConfig,
    system: &str,
    user: &str,
    max_tokens: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = chat_completions_url(&cfg.base_url);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cfg.timeout_seconds))
        .build()?;

    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "max_tokens": max_tokens,
        "temperature": 0.1
    });

    let mut last_err: Option<Box<dyn std::error::Error>> = None;
    for _ in 0..=cfg.max_retries {
        match do_chat_json_messages(&client, &url, &cfg.api_key, &body) {
            Ok(s) => return Ok(s),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| "unknown llm error".into()))
}

fn do_chat_json_messages(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    let resp = client.post(url).bearer_auth(api_key).json(body).send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("llm http {}: {}", status.as_u16(), text).into());
    }
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if content.is_empty() {
        return Err(format!("llm response missing content: {text}").into());
    }
    Ok(content)
}

pub fn smoke_chat_completion(
    cfg: &LlmConfig,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = chat_completions_url(&cfg.base_url);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cfg.timeout_seconds))
        .build()?;

    let mut last_err: Option<Box<dyn std::error::Error>> = None;
    for _ in 0..=cfg.max_retries {
        match do_chat_once(&client, &url, &cfg.api_key, &cfg.model, prompt) {
            Ok(s) => return Ok(s),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| "unknown llm error".into()))
}

/// OpenAI-compatible `/v1/embeddings`，使用 `AppConfig.embed`（需配置 `[embed]`）。
pub fn embed_texts(app: &AppConfig, input: &[String]) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    let embed = app
        .embed
        .as_ref()
        .ok_or("missing [embed] section in config (model required for embeddings)")?;
    let base = embed
        .base_url
        .as_deref()
        .unwrap_or(app.llm.base_url.as_str());
    let key = embed
        .api_key
        .as_deref()
        .unwrap_or(app.llm.api_key.as_str());
    let url = embeddings_url(base);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(app.llm.timeout_seconds))
        .build()?;

    let body = serde_json::json!({
        "model": embed.model,
        "input": input,
    });

    let mut last_err: Option<Box<dyn std::error::Error>> = None;
    for _ in 0..=app.llm.max_retries {
        match do_embed_once(&client, &url, key, &body) {
            Ok(v) => return Ok(v),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| "unknown embedding error".into()))
}

pub fn embed_first(app: &AppConfig, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let v = embed_texts(app, &[text.to_string()])?;
    v.into_iter().next().ok_or_else(|| "empty embedding response".into())
}

fn do_embed_once(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    let resp = client.post(url).bearer_auth(api_key).json(body).send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("embed http {}: {}", status.as_u16(), text).into());
    }
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let arr = v["data"]
        .as_array()
        .ok_or_else(|| format!("embed response missing data: {text}"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let emb = item["embedding"]
            .as_array()
            .ok_or_else(|| format!("embed missing embedding array: {text}"))?;
        let mut row = Vec::with_capacity(emb.len());
        for x in emb {
            let f = x
                .as_f64()
                .ok_or_else(|| format!("embed non-numeric: {text}"))? as f32;
            row.push(f);
        }
        out.push(row);
    }
    Ok(out)
}

fn embeddings_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/embeddings")
    } else {
        format!("{trimmed}/v1/embeddings")
    }
}

fn chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

fn do_chat_once(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 32,
        "temperature": 0.0
    });

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()?;

    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("llm http {}: {}", status.as_u16(), text).into());
    }

    let v: serde_json::Value = serde_json::from_str(&text)?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if content.is_empty() {
        return Err(format!("llm response missing content: {text}").into());
    }
    Ok(content)
}
