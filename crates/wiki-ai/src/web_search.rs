use crate::llm::{AppConfig, WebSearchPolicyConfig, WebSearchProviderConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchEvidence {
    pub query: String,
    pub provider: String,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub snippet: String,
    pub summary: Option<String>,
    pub retrieved_at: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchRun {
    pub query: String,
    pub providers_requested: Vec<String>,
    pub providers_succeeded: Vec<String>,
    pub providers_failed: Vec<WebSearchProviderFailure>,
    pub distinct_domains: usize,
    pub cross_verified: bool,
    pub evidence: Vec<WebSearchEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchProviderFailure {
    pub provider: String,
    pub error: String,
}

#[derive(Debug, Clone)]
struct ResolvedProvider {
    name: String,
    kind: String,
    api_key: String,
    model: Option<String>,
    tools: Vec<String>,
    max_results: usize,
    timeout_secs: u64,
}

pub fn run_search(
    app: &AppConfig,
    providers: &[String],
    query: &str,
) -> Result<WebSearchRun, Box<dyn std::error::Error>> {
    let web = app
        .web_search
        .as_ref()
        .ok_or("missing [web_search] config")?;
    let policy = web.policies.get("cross_verify");
    let requested_providers = requested_provider_names(policy, providers);
    if requested_providers.is_empty() {
        return Err("at least one web search provider is required".into());
    }
    let min_providers = policy
        .and_then(|policy| policy.min_providers)
        .unwrap_or(requested_providers.len().min(2));
    let min_domains = policy
        .and_then(|policy| policy.min_distinct_domains)
        .unwrap_or(2);
    let mut all = Vec::new();
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for name in &requested_providers {
        match resolve_provider(name, web.providers.get(name)) {
            Ok(provider) => match search_provider(&provider, query) {
                Ok(mut evidence) => {
                    if !evidence.is_empty() {
                        succeeded.push(name.clone());
                    }
                    all.append(&mut evidence);
                }
                Err(err) => failed.push(WebSearchProviderFailure {
                    provider: name.clone(),
                    error: sanitize_error(&err.to_string()),
                }),
            },
            Err(err) => failed.push(WebSearchProviderFailure {
                provider: name.clone(),
                error: sanitize_error(&err.to_string()),
            }),
        }
    }
    let evidence = dedupe_evidence(all);
    let domains: BTreeSet<_> = evidence.iter().map(|item| item.domain.clone()).collect();
    let cross_verified = succeeded.len() >= min_providers && domains.len() >= min_domains;
    Ok(WebSearchRun {
        query: query.to_string(),
        providers_requested: requested_providers,
        providers_succeeded: succeeded,
        providers_failed: failed,
        distinct_domains: domains.len(),
        cross_verified,
        evidence,
    })
}

fn requested_provider_names(
    policy: Option<&WebSearchPolicyConfig>,
    providers: &[String],
) -> Vec<String> {
    if providers.is_empty() {
        policy
            .map(|policy| policy.providers.clone())
            .unwrap_or_default()
    } else {
        providers.to_vec()
    }
}

fn resolve_provider(
    name: &str,
    provider: Option<&WebSearchProviderConfig>,
) -> Result<ResolvedProvider, Box<dyn std::error::Error>> {
    let provider = provider.ok_or_else(|| format!("unknown web search provider: {name}"))?;
    let mut api_key = provider.api_key.clone().unwrap_or_default();
    if let Some(env) = provider
        .api_key_env
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Ok(value) = std::env::var(env) {
            if !value.trim().is_empty() {
                api_key = value;
            }
        }
    }
    if api_key.trim().is_empty() {
        return Err(format!("web search provider {name} has no resolved api key").into());
    }
    Ok(ResolvedProvider {
        name: name.to_string(),
        kind: provider.kind.trim().to_ascii_lowercase(),
        api_key,
        model: provider
            .model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        tools: provider.tools.clone().unwrap_or_default(),
        max_results: provider.max_results.unwrap_or(8).clamp(1, 20),
        timeout_secs: provider.timeout_secs.unwrap_or(20).clamp(1, 120),
    })
}

fn search_provider(
    provider: &ResolvedProvider,
    query: &str,
) -> Result<Vec<WebSearchEvidence>, Box<dyn std::error::Error>> {
    match provider.kind.as_str() {
        "exa" => search_exa(provider, query),
        "tavily" => search_tavily(provider, query),
        "xai" => search_xai(provider, query),
        other => Err(format!("unsupported web search provider kind: {other}").into()),
    }
}

fn search_exa(
    provider: &ResolvedProvider,
    query: &str,
) -> Result<Vec<WebSearchEvidence>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()?;
    let body = serde_json::json!({
        "query": query,
        "numResults": provider.max_results,
        "contents": {
            "text": true,
            "summary": true
        }
    });
    let resp = client
        .post("https://api.exa.ai/search")
        .header("x-api-key", &provider.api_key)
        .json(&body)
        .send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("exa http {}: {}", status.as_u16(), sanitize_error(&text)).into());
    }
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let results = value["results"]
        .as_array()
        .ok_or("exa response missing results array")?;
    Ok(results
        .iter()
        .filter_map(|item| {
            let url = item["url"].as_str().or_else(|| item["id"].as_str())?;
            let title = item["title"].as_str().unwrap_or(url);
            let summary = item["summary"].as_str().map(ToString::to_string);
            let snippet = item["text"]
                .as_str()
                .or_else(|| item["highlight"].as_str())
                .or_else(|| item["highlights"][0].as_str())
                .or(summary.as_deref())
                .unwrap_or("")
                .to_string();
            Some(evidence_item(
                query,
                &provider.name,
                title,
                url,
                &snippet,
                summary,
            ))
        })
        .collect())
}

fn search_tavily(
    provider: &ResolvedProvider,
    query: &str,
) -> Result<Vec<WebSearchEvidence>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()?;
    let body = serde_json::json!({
        "query": query,
        "max_results": provider.max_results,
        "include_answer": false,
        "include_raw_content": false
    });
    let resp = client
        .post("https://api.tavily.com/search")
        .bearer_auth(&provider.api_key)
        .json(&body)
        .send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("tavily http {}: {}", status.as_u16(), sanitize_error(&text)).into());
    }
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let results = value["results"]
        .as_array()
        .ok_or("tavily response missing results array")?;
    Ok(results
        .iter()
        .filter_map(|item| {
            let url = item["url"].as_str()?;
            let title = item["title"].as_str().unwrap_or(url);
            let snippet = item["content"].as_str().unwrap_or("");
            Some(evidence_item(
                query,
                &provider.name,
                title,
                url,
                snippet,
                None,
            ))
        })
        .collect())
}

fn search_xai(
    provider: &ResolvedProvider,
    query: &str,
) -> Result<Vec<WebSearchEvidence>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_secs))
        .build()?;
    let tools = xai_tool_payloads(provider)?;
    let prompt = format!(
        "Search for current, source-backed evidence for this query. \
         Return a concise neutral summary and rely on source citations. Query: {query}"
    );
    let body = serde_json::json!({
        "model": provider.model.as_deref().unwrap_or("grok-4.3"),
        "input": [
            {
                "role": "user",
                "content": prompt
            }
        ],
        "tools": tools
    });
    let resp = client
        .post("https://api.x.ai/v1/responses")
        .bearer_auth(&provider.api_key)
        .json(&body)
        .send()?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        return Err(format!("xai http {}: {}", status.as_u16(), sanitize_error(&text)).into());
    }
    let value: serde_json::Value = serde_json::from_str(&text)?;
    extract_xai_evidence(provider, query, &value)
}

fn xai_tool_payloads(
    provider: &ResolvedProvider,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let configured = if provider.tools.is_empty() {
        vec!["web_search".to_string(), "x_search".to_string()]
    } else {
        provider.tools.clone()
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for tool in configured {
        let tool = tool.trim();
        if tool.is_empty() {
            continue;
        }
        match tool {
            "web_search" | "x_search" => {
                if seen.insert(tool.to_string()) {
                    out.push(serde_json::json!({ "type": tool }));
                }
            }
            other => {
                return Err(format!(
                    "unsupported xai search tool: {other}; only web_search and x_search are allowed"
                )
                .into())
            }
        }
    }
    if out.is_empty() {
        return Err("xai search provider requires at least one search tool".into());
    }
    Ok(out)
}

fn extract_xai_evidence(
    provider: &ResolvedProvider,
    query: &str,
    value: &serde_json::Value,
) -> Result<Vec<WebSearchEvidence>, Box<dyn std::error::Error>> {
    let output_text = collect_xai_output_text(value);
    let snippet = truncate_chars(output_text.trim(), 4_000);
    let summary = if snippet.is_empty() {
        None
    } else {
        Some(snippet.clone())
    };
    let citations = collect_xai_citations(value);
    if citations.is_empty() {
        return Err("xai response missing citations".into());
    }
    Ok(citations
        .into_iter()
        .take(provider.max_results)
        .map(|(url, title)| {
            let title = clean_xai_title(title.as_deref(), &url);
            evidence_item(
                query,
                &provider.name,
                &title,
                &url,
                &snippet,
                summary.clone(),
            )
        })
        .collect())
}

fn collect_xai_output_text(value: &serde_json::Value) -> String {
    if let Some(text) = value["output_text"].as_str() {
        return text.to_string();
    }
    let mut parts = Vec::new();
    if let Some(output) = value["output"].as_array() {
        for item in output {
            if let Some(content) = item["content"].as_array() {
                for block in content {
                    if let Some(text) = block["text"].as_str() {
                        parts.push(text.to_string());
                    }
                }
            }
        }
    }
    parts.join("\n")
}

fn collect_xai_citations(value: &serde_json::Value) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(output) = value["output"].as_array() {
        for item in output {
            if let Some(content) = item["content"].as_array() {
                for block in content {
                    if let Some(annotations) = block["annotations"].as_array() {
                        for annotation in annotations {
                            push_xai_citation(annotation, &mut out, &mut seen);
                        }
                    }
                }
            }
        }
    }
    if let Some(citations) = value["citations"].as_array() {
        for citation in citations {
            push_xai_citation(citation, &mut out, &mut seen);
        }
    }
    out
}

fn push_xai_citation(
    value: &serde_json::Value,
    out: &mut Vec<(String, Option<String>)>,
    seen: &mut BTreeSet<String>,
) {
    let url = value
        .as_str()
        .or_else(|| value["url"].as_str())
        .or_else(|| value["web_citation"]["url"].as_str())
        .or_else(|| value["x_citation"]["url"].as_str());
    let Some(url) = url.map(str::trim).filter(|url| !url.is_empty()) else {
        return;
    };
    if seen.insert(url.to_string()) {
        let title = value["title"]
            .as_str()
            .or_else(|| value["name"].as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string);
        out.push((url.to_string(), title));
    }
}

fn clean_xai_title(title: Option<&str>, url: &str) -> String {
    let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) else {
        return url.to_string();
    };
    if title.chars().all(|ch| ch.is_ascii_digit()) {
        url.to_string()
    } else {
        title.to_string()
    }
}

fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn evidence_item(
    query: &str,
    provider: &str,
    title: &str,
    url: &str,
    snippet: &str,
    summary: Option<String>,
) -> WebSearchEvidence {
    let domain = domain_from_url(url);
    let content_hash = evidence_hash(query, provider, title, url, snippet, summary.as_deref());
    WebSearchEvidence {
        query: query.to_string(),
        provider: provider.to_string(),
        title: title.to_string(),
        url: url.to_string(),
        domain,
        snippet: snippet.to_string(),
        summary,
        retrieved_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string()),
        content_hash,
    }
}

fn dedupe_evidence(items: Vec<WebSearchEvidence>) -> Vec<WebSearchEvidence> {
    let mut by_url: BTreeMap<String, WebSearchEvidence> = BTreeMap::new();
    for item in items {
        by_url
            .entry(item.url.clone())
            .and_modify(|existing| {
                if item.provider < existing.provider {
                    existing.provider = format!("{},{}", item.provider, existing.provider);
                } else if !existing.provider.split(',').any(|p| p == item.provider) {
                    existing.provider = format!("{},{}", existing.provider, item.provider);
                }
                if existing.snippet.is_empty() && !item.snippet.is_empty() {
                    existing.snippet = item.snippet.clone();
                }
                if existing.summary.is_none() {
                    existing.summary = item.summary.clone();
                }
            })
            .or_insert(item);
    }
    by_url.into_values().collect()
}

fn domain_from_url(url: &str) -> String {
    let rest = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url)
        .trim_start_matches('/');
    rest.split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn evidence_hash(
    query: &str,
    provider: &str,
    title: &str,
    url: &str,
    snippet: &str,
    summary: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    for part in [query, provider, title, url, snippet, summary.unwrap_or("")] {
        hasher.update(part.as_bytes());
        hasher.update(b"\0");
    }
    format!("sha256:v1:{}", hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn sanitize_error(text: &str) -> String {
    let (redacted, _) = wiki_core::redact_for_ingest(text);
    redacted.chars().take(500).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{AppConfig, LlmConfig, WebSearchConfig};

    fn app_config() -> AppConfig {
        let mut providers = BTreeMap::new();
        providers.insert(
            "exa".to_string(),
            WebSearchProviderConfig {
                kind: "exa".to_string(),
                api_key: Some("exa-key".to_string()),
                max_results: Some(8),
                timeout_secs: Some(20),
                ..Default::default()
            },
        );
        let mut policies = BTreeMap::new();
        policies.insert(
            "cross_verify".to_string(),
            WebSearchPolicyConfig {
                providers: vec!["exa".to_string(), "xai".to_string()],
                min_providers: Some(2),
                min_distinct_domains: Some(2),
            },
        );
        AppConfig {
            llm: LlmConfig {
                base_url: "https://api.example.test/v1".into(),
                api_key: "key".into(),
                api_key_env: None,
                model: "model".into(),
                timeout_seconds: 1,
                max_retries: 0,
                max_input_chars: crate::llm::DEFAULT_MAX_INPUT_CHARS,
                max_response_chars: crate::llm::DEFAULT_MAX_RESPONSE_CHARS,
                max_output_tokens: crate::llm::DEFAULT_MAX_OUTPUT_TOKENS,
                allowed_base_urls: Vec::new(),
                temperature: None,
                reasoning_effort: None,
                extra_body: None,
            },
            embed: None,
            llm_profiles: BTreeMap::new(),
            web_search: Some(WebSearchConfig {
                providers,
                policies,
            }),
        }
    }

    #[test]
    fn domain_extraction_handles_basic_https_url() {
        assert_eq!(
            domain_from_url("https://docs.example.com/a/b"),
            "docs.example.com"
        );
    }

    #[test]
    fn evidence_hash_is_stable_and_redaction_shape_is_prefixed() {
        let hash = evidence_hash("q", "exa", "t", "https://e.test", "s", Some("sum"));

        assert!(hash.starts_with("sha256:v1:"));
        assert_eq!(hash.len(), "sha256:v1:".len() + 64);
    }

    #[test]
    fn resolve_provider_errors_without_key() {
        let provider = WebSearchProviderConfig {
            kind: "exa".into(),
            ..Default::default()
        };

        let err = resolve_provider("exa", Some(&provider))
            .unwrap_err()
            .to_string();

        assert!(err.contains("no resolved api key"));
    }

    #[test]
    fn run_search_uses_cross_verify_policy_providers_when_omitted() {
        let app = app_config();

        let providers = requested_provider_names(
            app.web_search
                .as_ref()
                .and_then(|web| web.policies.get("cross_verify")),
            &[],
        );

        assert_eq!(providers, vec!["exa", "xai"]);
    }

    #[test]
    fn dedupe_evidence_merges_same_url_from_multiple_providers() {
        let a = evidence_item("q", "exa", "Title", "https://example.test/a", "a", None);
        let b = evidence_item(
            "q",
            "xai",
            "Title",
            "https://example.test/a",
            "b",
            Some("summary".into()),
        );

        let out = dedupe_evidence(vec![a, b]);

        assert_eq!(out.len(), 1);
        assert!(out[0].provider.contains("exa"));
        assert!(out[0].provider.contains("xai"));
        assert_eq!(out[0].summary.as_deref(), Some("summary"));
    }

    #[test]
    fn xai_tool_payloads_default_to_web_and_x_search() {
        let provider = ResolvedProvider {
            name: "xai".into(),
            kind: "xai".into(),
            api_key: "key".into(),
            model: Some("grok-4.3".into()),
            tools: Vec::new(),
            max_results: 8,
            timeout_secs: 20,
        };

        let tools = xai_tool_payloads(&provider).unwrap();

        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["type"], "web_search");
        assert_eq!(tools[1]["type"], "x_search");
    }

    #[test]
    fn xai_tool_payloads_reject_code_interpreter() {
        let provider = ResolvedProvider {
            name: "xai".into(),
            kind: "xai".into(),
            api_key: "key".into(),
            model: Some("grok-4.3".into()),
            tools: vec!["web_search".into(), "code_interpreter".into()],
            max_results: 8,
            timeout_secs: 20,
        };

        let err = xai_tool_payloads(&provider).unwrap_err().to_string();

        assert!(err.contains("unsupported xai search tool"));
    }

    #[test]
    fn xai_response_extracts_citations_as_evidence() {
        let provider = ResolvedProvider {
            name: "xai".into(),
            kind: "xai".into(),
            api_key: "key".into(),
            model: Some("grok-4.3".into()),
            tools: Vec::new(),
            max_results: 8,
            timeout_secs: 20,
        };
        let value = serde_json::json!({
            "output_text": "xAI summary with source-backed evidence.",
            "citations": [
                "https://alpha.example/report"
            ],
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "xAI summary with source-backed evidence.",
                    "annotations": [{
                        "type": "url_citation",
                        "url": "https://beta.example/article",
                        "title": "Beta article",
                        "start_index": 0,
                        "end_index": 5
                    }]
                }]
            }]
        });

        let out = extract_xai_evidence(&provider, "query", &value).unwrap();

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].provider, "xai");
        assert_eq!(out[0].domain, "beta.example");
        assert_eq!(out[0].title, "Beta article");
        assert_eq!(out[1].domain, "alpha.example");
        assert_eq!(
            out[0].summary.as_deref(),
            Some("xAI summary with source-backed evidence.")
        );
    }
}
