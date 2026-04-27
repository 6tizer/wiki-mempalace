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
  "summary": {
    "title": "short title for the summary page",
    "one_sentence_summary": "one sentence TL;DR in the source language",
    "key_insights": [ "3-5 bullet-sized insight strings" ],
    "confidence": "high|medium|low",
    "tags": [ "2-3 wiki tags across scenario/method/product dimensions when possible" ],
    "personal_note": "how this relates to Tizer's workflow, or empty string",
    "source_author": "author if identifiable in text, else null",
    "source_publisher": "platform or publisher if identifiable, else null",
    "source_published_at": "publication time if identifiable, else null",
    "external_url": "source article URL if available, else null"
  },
  "summary_title": "short title for a wiki page (same language as source)",
  "summary_markdown": "optional extra markdown; if one_sentence_summary is set, put supporting detail here",
  "one_sentence_summary": "one sentence TL;DR in the source language",
  "key_insights": [ "bullet-sized insight strings in the source language" ],
  "confidence": "high|medium|low",
  "tags": [ "short source/summary wiki tags in the source language" ],
  "source_author": "author if identifiable in text, else null",
  "source_publisher": "platform or publisher if identifiable, else null",
  "source_published_at": "publication time if identifiable, else null",
  "claims": [ { "text": "atomic factual claim in the same language as the source", "tier": "semantic", "tags": [ "short claim-specific wiki tags" ] } ],
  "concepts": [
    {
      "canonical_name": "standard concept page title",
      "kind": "concept",
      "definition": "one concise Chinese definition",
      "key_points": [ "2-5 useful points" ],
      "tags": [ "2-3 wiki tags" ],
      "related_names": [ "related concept/entity names" ],
      "category": "scenario|method|product|null"
    }
  ],
  "entities": [
    {
      "label": "EntityName",
      "kind": "person|project|library|file_path|decision|other|concept",
      "canonical_name": "standard entity page title",
      "category": "product|company|person|project|library|model|tool|null",
      "definition": "definition if this is concept-like",
      "profile": "entity profile if this is a concrete product/company/person/project",
      "key_points": [ "2-5 useful points" ],
      "tags": [ "2-3 wiki tags" ],
      "related_names": [ "related concept/entity names" ]
    }
  ],
  "relationships": [ { "from_label": "EntityA", "relation": "uses", "to_label": "EntityB" } ]
}
Rules:
- "tier" must be one of: working, episodic, semantic, procedural
- "kind" must be one of: person, project, library, concept, file_path, decision, other
- "relation" must be one of: uses, depends_on, contradicts, caused, fixed, supersedes, related
- Create one summary and extract visible wiki entries.
- concepts: target 3-7 per detailed article; 3-4 is fine for short articles. Avoid overly generic names like "AI", "工具", "效率".
- entities: concrete products, companies, people, projects, libraries, models, and tools. If the item is a method or idea, put it in concepts instead.
- claims: 0–12 items, each one short standalone sentence. Claims support search but do not replace concept/entity pages.
- claim tags: optional, 0–6 items per claim, specific to that claim
- relationships: 0–10 items, typed directed edges between entities
- key_insights: 0–8 items
- top-level tags: 0–12 items, source/summary-level only
- confidence must be exactly one of: high, medium, low
- Prefer filling one_sentence_summary + key_insights; use summary_markdown only when needed for nuance
- Prefer Chinese content. Keep official English product names and exact capitalization when appropriate.
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
pub fn embed_texts(
    app: &AppConfig,
    input: &[String],
) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
    let embed = app
        .embed
        .as_ref()
        .ok_or("missing [embed] section in config (model required for embeddings)")?;
    let base = embed
        .base_url
        .as_deref()
        .unwrap_or(app.llm.base_url.as_str());
    let key = embed.api_key.as_deref().unwrap_or(app.llm.api_key.as_str());
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
    v.into_iter()
        .next()
        .ok_or_else(|| "empty embedding response".into())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_ingest_prompt_describes_claim_level_tags() {
        let prompt = ingest_llm_system_prompt();

        assert!(prompt.contains("\"tags\": [ \"short source/summary wiki tags"));
        assert!(prompt.contains("\"tags\": [ \"short claim-specific wiki tags\" ]"));
        assert!(prompt.contains("claim tags: optional"));
        assert!(prompt.contains("top-level tags: 0"));
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
        "max_tokens": 128,
        "temperature": 0.0
    });

    let resp = client.post(url).bearer_auth(api_key).json(&body).send()?;

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
