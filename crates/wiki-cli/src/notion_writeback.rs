//! Notion write-back client: marks pages as compiled in Notion.
//!
//! The trait is the extension point; `NoopWriteBack` is the default and
//! is always safe to use. `HttpNotionWriteBack` is provided for when
//! `--writeback-notion` is enabled but is disabled by default.

use reqwest::blocking::Client;

const NOTION_API_BASE: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";

#[derive(Debug, thiserror::Error)]
pub enum WriteBackError {
    #[error("HTTP write-back failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Notion API write-back error: {0}")]
    Api(String),
}

/// Extension point for writing back to Notion after a page is ingested.
pub trait NotionWriteBackClient: Send + Sync {
    /// Mark a Notion page as compiled into the local wiki.
    ///
    /// Implementations should return `Err` on failure rather than panicking.
    /// Callers treat errors as warnings and do not abort the sync.
    fn mark_compiled(&self, page_id: &str) -> Result<(), WriteBackError>;
}

/// Default no-op implementation. Does nothing.
pub struct NoopWriteBack;

impl NotionWriteBackClient for NoopWriteBack {
    fn mark_compiled(&self, _page_id: &str) -> Result<(), WriteBackError> {
        Ok(())
    }
}

/// Live implementation that sets the `已编译到Wiki` checkbox on the Notion page
/// via `PATCH /v1/pages/{page_id}`.
pub struct HttpNotionWriteBack {
    token: String,
    client: Client,
    /// Override the API base URL for testing.
    base_url: String,
}

impl HttpNotionWriteBack {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            client: Client::new(),
            base_url: NOTION_API_BASE.to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

impl NotionWriteBackClient for HttpNotionWriteBack {
    fn mark_compiled(&self, page_id: &str) -> Result<(), WriteBackError> {
        let url = format!("{}/pages/{}", self.base_url, page_id);
        let body = serde_json::json!({
            "properties": {
                "已编译到Wiki": {"checkbox": true}
            }
        });

        let resp = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", NOTION_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(WriteBackError::Api(format!("HTTP {status}: {text}")));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[test]
    fn notion_writeback_noop_always_ok() {
        let wb = NoopWriteBack;
        assert!(wb.mark_compiled("any-page-id").is_ok());
        assert!(wb.mark_compiled("").is_ok());
    }

    #[test]
    fn notion_writeback_http_success() {
        let mut server = Server::new();
        let _m = server
            .mock("PATCH", "/pages/test-page-id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": "test-page-id"}"#)
            .create();

        let wb = HttpNotionWriteBack::new("test-token").with_base_url(server.url());
        assert!(wb.mark_compiled("test-page-id").is_ok());
    }

    #[test]
    fn notion_writeback_http_api_error_returns_err_not_panic() {
        let mut server = Server::new();
        let _m = server
            .mock("PATCH", "/pages/bad-page-id")
            .with_status(403)
            .with_body(r#"{"message": "Unauthorized"}"#)
            .create();

        let wb = HttpNotionWriteBack::new("bad-token").with_base_url(server.url());
        let result = wb.mark_compiled("bad-page-id");
        assert!(result.is_err());
        match result {
            Err(WriteBackError::Api(msg)) => assert!(msg.contains("403")),
            other => panic!("expected Api error, got {other:?}"),
        }
    }
}
