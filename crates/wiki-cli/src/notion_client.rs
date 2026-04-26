//! Notion API HTTP client for incremental sync.
//!
//! Reads NOTION_TOKEN from the environment; never prints the token.
//! Implements per-request rate limiting (350 ms default) and HTTP 429
//! back-off with up to 3 retries.

use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

const NOTION_API_BASE: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";
const DEFAULT_REQUEST_DELAY_MS: u64 = 350;
const MIN_REQUEST_DELAY_MS: u64 = 100;
const MAX_RETRIES: u32 = 3;
const DEFAULT_RETRY_AFTER_SECS: u64 = 60;

#[derive(Debug, thiserror::Error)]
pub enum NotionClientError {
    #[error("NOTION_TOKEN environment variable is not set")]
    TokenNotSet,
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("rate limit exceeded after {MAX_RETRIES} retries")]
    RateLimitExceeded,
    #[error("unexpected API response: {0}")]
    Api(String),
}

/// A single page record returned by the Notion database query API.
#[derive(Debug, Clone)]
pub struct NotionPage {
    /// Notion page UUID (hyphenated, e.g. "abc-123-...")
    pub id: String,
    #[allow(dead_code)]
    pub last_edited_time: OffsetDateTime,
    pub title: String,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub note: Option<String>,
    pub status: Option<String>,
    pub content: String,
}

pub struct NotionApiClient {
    token: String,
    client: Client,
    request_delay: Duration,
    last_request_at: Option<Instant>,
    /// Override the API base URL for testing.
    base_url: String,
}

impl NotionApiClient {
    pub fn from_env() -> Result<Self, NotionClientError> {
        Self::from_env_with_delay(DEFAULT_REQUEST_DELAY_MS)
    }

    pub fn from_env_with_delay(request_delay_ms: u64) -> Result<Self, NotionClientError> {
        let token = std::env::var("NOTION_TOKEN").map_err(|_| NotionClientError::TokenNotSet)?;
        let delay_ms = request_delay_ms.max(MIN_REQUEST_DELAY_MS);
        Ok(Self {
            token,
            client: Client::new(),
            request_delay: Duration::from_millis(delay_ms),
            last_request_at: None,
            base_url: NOTION_API_BASE.to_string(),
        })
    }

    /// Override the request delay (useful when the CLI passes `--request-delay-ms`).
    pub fn with_request_delay_ms(mut self, ms: u64) -> Self {
        self.request_delay = Duration::from_millis(ms.max(MIN_REQUEST_DELAY_MS));
        self
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Query a Notion database incrementally.
    ///
    /// Returns pages with `last_edited_time >= since` (or all pages if `since` is None).
    /// At most `limit` pages are returned across all pagination cursors.
    pub fn query_database_incremental(
        &mut self,
        db_id: &str,
        since: Option<OffsetDateTime>,
        limit: Option<usize>,
    ) -> Result<Vec<NotionPage>, NotionClientError> {
        let mut pages: Vec<NotionPage> = Vec::new();
        let mut start_cursor: Option<String> = None;

        let since_str = since.map(|t| {
            t.format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default()
        });

        loop {
            let body = build_query_body(since_str.as_deref(), start_cursor.as_deref());
            let url = format!("{}/databases/{}/query", self.base_url, db_id);

            let response_json = self.post_with_retry(&url, &body)?;

            let batch = parse_query_response(&response_json)?;
            let has_more = batch.has_more;
            let next_cursor = batch.next_cursor;

            for raw in batch.results {
                if let Some(mut page) = parse_notion_page(raw) {
                    page.content = self.fetch_page_content(&page.id)?;
                    pages.push(page);
                    if let Some(lim) = limit {
                        if pages.len() >= lim {
                            return Ok(pages);
                        }
                    }
                }
            }

            if !has_more || next_cursor.is_none() {
                break;
            }
            start_cursor = next_cursor;
        }

        Ok(pages)
    }

    fn post_with_retry(
        &mut self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, NotionClientError> {
        let mut retries = 0u32;
        loop {
            self.rate_limit_sleep();
            let resp = self
                .client
                .post(url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Notion-Version", NOTION_VERSION)
                .header("Content-Type", "application/json")
                .json(body)
                .send()?;

            let status = resp.status();
            if status.as_u16() == 429 {
                if retries >= MAX_RETRIES {
                    return Err(NotionClientError::RateLimitExceeded);
                }
                let retry_after = retry_after_secs(&resp);
                std::thread::sleep(Duration::from_secs(retry_after));
                retries += 1;
                continue;
            }

            if !status.is_success() {
                let text = resp.text().unwrap_or_default();
                return Err(NotionClientError::Api(format!("HTTP {status}: {text}")));
            }

            return Ok(resp.json()?);
        }
    }

    fn get_with_retry(&mut self, url: &str) -> Result<serde_json::Value, NotionClientError> {
        let mut retries = 0u32;
        loop {
            self.rate_limit_sleep();
            let resp = self
                .client
                .get(url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Notion-Version", NOTION_VERSION)
                .send()?;

            let status = resp.status();
            if status.as_u16() == 429 {
                if retries >= MAX_RETRIES {
                    return Err(NotionClientError::RateLimitExceeded);
                }
                let retry_after = retry_after_secs(&resp);
                std::thread::sleep(Duration::from_secs(retry_after));
                retries += 1;
                continue;
            }

            if !status.is_success() {
                let text = resp.text().unwrap_or_default();
                return Err(NotionClientError::Api(format!("HTTP {status}: {text}")));
            }

            return Ok(resp.json()?);
        }
    }

    fn fetch_page_content(&mut self, page_id: &str) -> Result<String, NotionClientError> {
        let mut blocks = Vec::new();
        let mut start_cursor: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/blocks/{}/children?page_size=100",
                self.base_url, page_id
            );
            if let Some(cursor) = &start_cursor {
                url.push_str("&start_cursor=");
                url.push_str(cursor);
            }

            let response_json = self.get_with_retry(&url)?;
            let batch = parse_block_children_response(&response_json)?;
            for raw in batch.results {
                if let Some(text) = render_block_text(&raw) {
                    blocks.push(text);
                }
            }

            if !batch.has_more || batch.next_cursor.is_none() {
                break;
            }
            start_cursor = batch.next_cursor;
        }

        Ok(blocks.join("\n\n"))
    }

    fn rate_limit_sleep(&mut self) {
        if let Some(last) = self.last_request_at {
            let elapsed = last.elapsed();
            if elapsed < self.request_delay {
                std::thread::sleep(self.request_delay - elapsed);
            }
        }
        self.last_request_at = Some(Instant::now());
    }
}

fn retry_after_secs(resp: &reqwest::blocking::Response) -> u64 {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_RETRY_AFTER_SECS)
}

fn build_query_body(since: Option<&str>, start_cursor: Option<&str>) -> serde_json::Value {
    let mut body = serde_json::json!({
        "page_size": 100,
        "sorts": [{"timestamp": "last_edited_time", "direction": "descending"}]
    });

    if let Some(since_str) = since {
        body["filter"] = serde_json::json!({
            "timestamp": "last_edited_time",
            "last_edited_time": {"on_or_after": since_str}
        });
    }

    if let Some(cursor) = start_cursor {
        body["start_cursor"] = serde_json::json!(cursor);
    }

    body
}

// --- Internal response deserialization ---

#[derive(Debug, Deserialize)]
struct QueryResponse {
    results: Vec<serde_json::Value>,
    has_more: bool,
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BlockChildrenResponse {
    results: Vec<serde_json::Value>,
    has_more: bool,
    next_cursor: Option<String>,
}

fn parse_query_response(json: &serde_json::Value) -> Result<QueryResponse, NotionClientError> {
    serde_json::from_value(json.clone())
        .map_err(|e| NotionClientError::Api(format!("failed to parse query response: {e}")))
}

fn parse_block_children_response(
    json: &serde_json::Value,
) -> Result<BlockChildrenResponse, NotionClientError> {
    serde_json::from_value(json.clone()).map_err(|e| {
        NotionClientError::Api(format!("failed to parse block children response: {e}"))
    })
}

fn parse_notion_page(raw: serde_json::Value) -> Option<NotionPage> {
    let id = raw["id"].as_str()?.to_string();

    let last_edited_str = raw["last_edited_time"].as_str()?;
    let last_edited_time = OffsetDateTime::parse(
        last_edited_str,
        &time::format_description::well_known::Rfc3339,
    )
    .ok()?;

    let props = &raw["properties"];

    let title = extract_title(props);
    let url = extract_url(props, "文章链接");
    let tags = extract_multi_select(props, "标签");
    let source = extract_select(props, "来源");
    let note = extract_rich_text(props, "备注");
    let status = extract_select(props, "状态");

    Some(NotionPage {
        id,
        last_edited_time,
        title,
        url,
        tags,
        source,
        note,
        status,
        content: String::new(),
    })
}

fn render_block_text(raw: &serde_json::Value) -> Option<String> {
    let block_type = raw["type"].as_str()?;
    let block = &raw[block_type];
    let text = rich_text_plain(block);
    let text = text.trim();

    let rendered = match block_type {
        "paragraph" => text.to_string(),
        "heading_1" => format!("# {text}"),
        "heading_2" => format!("## {text}"),
        "heading_3" => format!("### {text}"),
        "bulleted_list_item" => format!("- {text}"),
        "numbered_list_item" => format!("1. {text}"),
        "to_do" => {
            let checked = block["checked"].as_bool().unwrap_or(false);
            format!("- [{}] {text}", if checked { "x" } else { " " })
        }
        "quote" => format!("> {text}"),
        "code" => {
            let language = block["language"].as_str().unwrap_or("");
            format!("```{language}\n{text}\n```")
        }
        "callout" => format!("> {text}"),
        "toggle" => format!("- {text}"),
        "bookmark" | "link_preview" => block["url"].as_str().unwrap_or("").to_string(),
        _ => text.to_string(),
    };

    if rendered.trim().is_empty() {
        None
    } else {
        Some(rendered)
    }
}

fn rich_text_plain(block: &serde_json::Value) -> String {
    block["rich_text"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v["plain_text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

fn extract_title(props: &serde_json::Value) -> String {
    props["Name"]["title"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["plain_text"].as_str())
        .unwrap_or("")
        .to_string()
}

fn extract_url(props: &serde_json::Value, key: &str) -> Option<String> {
    props[key]["url"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn extract_multi_select(props: &serde_json::Value, key: &str) -> Vec<String> {
    props[key]["multi_select"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v["name"].as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn extract_select(props: &serde_json::Value, key: &str) -> Option<String> {
    props[key]["select"]["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn extract_rich_text(props: &serde_json::Value, key: &str) -> Option<String> {
    let text = props[key]["rich_text"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v["plain_text"].as_str())
        .unwrap_or("")
        .to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    fn make_page_json(page_id: &str, last_edited: &str, title: &str) -> serde_json::Value {
        serde_json::json!({
            "id": page_id,
            "last_edited_time": last_edited,
            "properties": {
                "Name": {"title": [{"plain_text": title}]},
                "文章链接": {"url": "https://example.com"},
                "标签": {"multi_select": [{"name": "LLM"}]},
                "来源": {"select": {"name": "X"}},
                "备注": {"rich_text": [{"plain_text": "some note"}]},
                "状态": {"select": {"name": "待读"}}
            }
        })
    }

    fn mock_blocks(server: &mut Server, page_id: &str, text: &str) -> mockito::Mock {
        server
            .mock("GET", format!("/blocks/{page_id}/children").as_str())
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "results": [{
                        "type": "paragraph",
                        "paragraph": {"rich_text": [{"plain_text": text}]}
                    }],
                    "has_more": false,
                    "next_cursor": null
                })
                .to_string(),
            )
            .create()
    }

    #[test]
    fn notion_client_pagination() {
        let mut server = Server::new();

        let page1 = make_page_json("page-1", "2026-04-01T00:00:00.000Z", "Title 1");
        let page2 = make_page_json("page-2", "2026-04-02T00:00:00.000Z", "Title 2");
        let _b1 = mock_blocks(&mut server, "page-1", "Body 1");
        let _b2 = mock_blocks(&mut server, "page-2", "Body 2");

        // First request: has_more = true
        let _m1 = server
            .mock("POST", "/databases/test-db/query")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "results": [page1],
                    "has_more": true,
                    "next_cursor": "cursor-abc"
                })
                .to_string(),
            )
            .create();

        // Second request with start_cursor
        let _m2 = server
            .mock("POST", "/databases/test-db/query")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "results": [page2],
                    "has_more": false,
                    "next_cursor": null
                })
                .to_string(),
            )
            .create();

        unsafe { std::env::set_var("NOTION_TOKEN", "test-token") };
        let mut client = NotionApiClient::from_env_with_delay(0)
            .unwrap()
            .with_base_url(server.url());

        let pages = client
            .query_database_incremental("test-db", None, None)
            .unwrap();

        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].id, "page-1");
        assert_eq!(pages[1].id, "page-2");
        assert_eq!(pages[0].tags, vec!["LLM"]);
        assert_eq!(pages[0].url, Some("https://example.com".to_string()));
        assert_eq!(pages[0].content, "Body 1");
    }

    #[test]
    fn notion_client_rate_limit() {
        let mut server = Server::new();

        let page = make_page_json("page-1", "2026-04-01T00:00:00.000Z", "Title 1");
        let _b1 = mock_blocks(&mut server, "page-1", "Body 1");

        // First two requests return 429
        let _m1 = server
            .mock("POST", "/databases/test-db/query")
            .with_status(429)
            .with_header("retry-after", "0")
            .create();
        let _m2 = server
            .mock("POST", "/databases/test-db/query")
            .with_status(429)
            .with_header("retry-after", "0")
            .create();
        // Third request succeeds
        let _m3 = server
            .mock("POST", "/databases/test-db/query")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "results": [page],
                    "has_more": false,
                    "next_cursor": null
                })
                .to_string(),
            )
            .create();

        unsafe { std::env::set_var("NOTION_TOKEN", "test-token") };
        let mut client = NotionApiClient::from_env_with_delay(0)
            .unwrap()
            .with_base_url(server.url());

        let pages = client
            .query_database_incremental("test-db", None, None)
            .unwrap();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "page-1");
    }

    #[test]
    fn notion_client_rate_limit_exceeded() {
        let mut server = Server::new();

        // All 4 requests (initial + 3 retries) return 429
        for _ in 0..=MAX_RETRIES {
            server
                .mock("POST", "/databases/test-db/query")
                .with_status(429)
                .with_header("retry-after", "0")
                .create();
        }

        unsafe { std::env::set_var("NOTION_TOKEN", "test-token") };
        let mut client = NotionApiClient::from_env_with_delay(0)
            .unwrap()
            .with_base_url(server.url());

        let result = client.query_database_incremental("test-db", None, None);
        assert!(matches!(result, Err(NotionClientError::RateLimitExceeded)));
    }

    #[test]
    fn notion_client_limit_truncates_results() {
        let mut server = Server::new();

        let pages: Vec<serde_json::Value> = (1..=5)
            .map(|i| make_page_json(&format!("page-{i}"), "2026-04-01T00:00:00.000Z", "T"))
            .collect();
        let _block_mocks: Vec<_> = (1..=3)
            .map(|i| mock_blocks(&mut server, &format!("page-{i}"), &format!("Body {i}")))
            .collect();

        let _m = server
            .mock("POST", "/databases/test-db/query")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "results": pages,
                    "has_more": false,
                    "next_cursor": null
                })
                .to_string(),
            )
            .create();

        unsafe { std::env::set_var("NOTION_TOKEN", "test-token") };
        let mut client = NotionApiClient::from_env_with_delay(0)
            .unwrap()
            .with_base_url(server.url());

        let result = client
            .query_database_incremental("test-db", None, Some(3))
            .unwrap();

        assert_eq!(result.len(), 3);
    }

    #[test]
    fn notion_client_token_not_set_returns_error() {
        unsafe { std::env::remove_var("NOTION_TOKEN") };
        let result = NotionApiClient::from_env();
        assert!(matches!(result, Err(NotionClientError::TokenNotSet)));
        // Restore for other tests
        unsafe { std::env::set_var("NOTION_TOKEN", "test-token") };
    }
}
