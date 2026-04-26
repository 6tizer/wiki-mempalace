//! Core Notion incremental sync logic.
//!
//! `NotionSyncRunner` orchestrates the full sync flow for a single Notion DB:
//! read cursor → fetch incremental pages → deduplicate → ingest → update cursor.

use crate::notion_client::{NotionApiClient, NotionClientError, NotionPage};
use crate::notion_writeback::NotionWriteBackClient;
use std::time::Instant;
use time::OffsetDateTime;
use wiki_core::{DomainSchema, Scope};
use wiki_kernel::{EngineError, LlmWikiEngine, NoopWikiHook};
use wiki_storage::{SqliteRepository, WikiRepository};

/// The default look-back window for the first sync (no cursor stored).
const DEFAULT_LOOKBACK_DAYS: i64 = 30;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("Notion API error: {0}")]
    Api(#[from] NotionClientError),
    #[error("storage error: {0}")]
    Storage(#[from] wiki_storage::StorageError),
    #[error("engine error: {0}")]
    Engine(#[from] EngineError),
}

/// Summary of a single sync run.
#[derive(Debug, Clone, Default)]
pub struct SyncResult {
    pub db_id: String,
    /// Pages returned from the Notion API.
    pub fetched: usize,
    /// Pages newly ingested into wiki.db.
    pub new: usize,
    /// Pages skipped because their `notion_page_id` already exists.
    pub skipped: usize,
    /// Pages that failed to ingest (non-fatal; logged but not retried).
    pub errors: usize,
    pub duration_secs: f64,
}

pub struct NotionSyncRunner<'a> {
    client: &'a mut NotionApiClient,
    repo: &'a SqliteRepository,
    engine: &'a mut LlmWikiEngine<NoopWikiHook>,
    scope: Scope,
    verbose: bool,
}

impl<'a> NotionSyncRunner<'a> {
    pub fn new(
        client: &'a mut NotionApiClient,
        repo: &'a SqliteRepository,
        engine: &'a mut LlmWikiEngine<NoopWikiHook>,
        scope: Scope,
        verbose: bool,
    ) -> Self {
        Self {
            client,
            repo,
            engine,
            scope,
            verbose,
        }
    }

    /// Run incremental sync for a single Notion database.
    ///
    /// `db_id` is the local slug ("x_bookmark" or "wechat").
    /// `notion_db_id` is the Notion UUID for the database.
    pub fn run_sync(
        &mut self,
        db_id: &str,
        notion_db_id: &str,
        since_override: Option<OffsetDateTime>,
        limit: Option<usize>,
        dry_run: bool,
        writeback: &dyn NotionWriteBackClient,
    ) -> Result<SyncResult, SyncError> {
        let started = Instant::now();
        let sync_started_at = OffsetDateTime::now_utc();

        let since = if let Some(ov) = since_override {
            Some(ov)
        } else {
            match self.repo.get_notion_sync_cursor(db_id)? {
                Some(cursor) => Some(cursor),
                None => {
                    let days = time::Duration::days(DEFAULT_LOOKBACK_DAYS);
                    Some(sync_started_at - days)
                }
            }
        };

        let pages = self
            .client
            .query_database_incremental(notion_db_id, since, limit)?;

        let mut result = SyncResult {
            db_id: db_id.to_string(),
            fetched: pages.len(),
            ..Default::default()
        };

        for page in &pages {
            let already_exists = self.repo.notion_page_exists(&page.id)?;
            if already_exists {
                result.skipped += 1;
                if self.verbose {
                    eprintln!(
                        "notion-sync: skipping existing page_id={} title={}",
                        page.id, page.title
                    );
                }
                continue;
            }

            if dry_run {
                result.new += 1;
                continue;
            }

            match self.ingest_page(db_id, page) {
                Ok(source_id) => {
                    self.repo
                        .insert_notion_page_index(&page.id, db_id, &source_id)?;

                    if let Err(e) = writeback.mark_compiled(&page.id) {
                        eprintln!(
                            "notion-sync: writeback warn page_id={}: {e}",
                            page.id
                        );
                    }

                    result.new += 1;
                    if self.verbose {
                        eprintln!(
                            "notion-sync: ingested page_id={} source_id={} title={}",
                            page.id, source_id.0, page.title
                        );
                    }
                }
                Err(e) => {
                    eprintln!("notion-sync: error page_id={}: {e}", page.id);
                    result.errors += 1;
                }
            }
        }

        if !dry_run && result.errors == 0 {
            self.engine.save_to_repo_and_flush_outbox(self.repo)?;
            self.repo
                .upsert_notion_sync_cursor(db_id, sync_started_at, result.new as i64)?;
        }

        result.duration_secs = started.elapsed().as_secs_f64();
        Ok(result)
    }

    fn ingest_page(
        &mut self,
        db_id: &str,
        page: &NotionPage,
    ) -> Result<wiki_core::SourceId, SyncError> {
        let uri = format!("notion://{}/{}", db_id, page.id);
        let body = build_body(page);
        let source_id = self
            .engine
            .ingest_raw_with_tags(&uri, &body, self.scope.clone(), "notion-sync", &page.tags)?;
        Ok(source_id)
    }
}

/// Build the plain-text body for a Notion page ingest.
fn build_body(page: &NotionPage) -> String {
    let mut parts = vec![format!("# {}", page.title), String::new()];

    if let Some(url) = &page.url {
        parts.push(format!("URL: {url}"));
    }
    if let Some(source) = &page.source {
        parts.push(format!("来源: {source}"));
    }
    if let Some(status) = &page.status {
        parts.push(format!("状态: {status}"));
    }
    if let Some(note) = &page.note {
        parts.push(format!("备注: {note}"));
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notion_client::NotionPage;
    use crate::notion_writeback::NoopWriteBack;
    use tempfile::tempdir;
    use time::OffsetDateTime;
    use wiki_storage::SqliteRepository;

    fn private_scope() -> Scope {
        Scope::Private {
            agent_id: "test".to_string(),
        }
    }

    fn make_page(id: &str, title: &str) -> NotionPage {
        NotionPage {
            id: id.to_string(),
            last_edited_time: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            title: title.to_string(),
            url: Some("https://example.com".to_string()),
            tags: vec!["LLM".to_string()],
            source: Some("X".to_string()),
            note: None,
            status: Some("待读".to_string()),
        }
    }

    // Test helper: run sync logic with injected pages (no real HTTP).
    fn run_sync_with_pages(
        db_id: &str,
        pages: Vec<NotionPage>,
        since_override: Option<OffsetDateTime>,
        limit: Option<usize>,
        dry_run: bool,
        repo: &SqliteRepository,
        engine: &mut LlmWikiEngine<NoopWikiHook>,
        scope: Scope,
    ) -> Result<SyncResult, SyncError> {
        let sync_started_at = OffsetDateTime::now_utc();
        let started = Instant::now();

        let since = if since_override.is_some() {
            since_override
        } else {
            match repo.get_notion_sync_cursor(db_id)? {
                Some(cursor) => Some(cursor),
                None => {
                    let days = time::Duration::days(DEFAULT_LOOKBACK_DAYS);
                    Some(sync_started_at - days)
                }
            }
        };
        let _ = since; // used in real path; stub skips API call

        // Apply limit
        let pages: Vec<_> = match limit {
            Some(n) => pages.into_iter().take(n).collect(),
            None => pages,
        };

        let mut result = SyncResult {
            db_id: db_id.to_string(),
            fetched: pages.len(),
            ..Default::default()
        };

        let wb = NoopWriteBack;

        for page in &pages {
            let already_exists = repo.notion_page_exists(&page.id)?;
            if already_exists {
                result.skipped += 1;
                continue;
            }

            if dry_run {
                result.new += 1;
                continue;
            }

            let uri = format!("notion://{}/{}", db_id, page.id);
            let body = build_body(page);
            match engine.ingest_raw_with_tags(&uri, &body, scope.clone(), "notion-sync", &page.tags)
            {
                Ok(source_id) => {
                    repo.insert_notion_page_index(&page.id, db_id, &source_id)?;
                    let _ = wb.mark_compiled(&page.id);
                    result.new += 1;
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    result.errors += 1;
                }
            }
        }

        if !dry_run && result.errors == 0 {
            engine.save_to_repo_and_flush_outbox(repo)?;
            repo.upsert_notion_sync_cursor(db_id, sync_started_at, result.new as i64)?;
        }

        result.duration_secs = started.elapsed().as_secs_f64();
        Ok(result)
    }

    #[test]
    fn notion_sync_ingests_new_page() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut engine = LlmWikiEngine::new(schema);

        let pages = vec![make_page("page-abc", "Test Page")];
        let result = run_sync_with_pages(
            "x_bookmark",
            pages,
            None,
            None,
            false,
            &repo,
            &mut engine,
            private_scope(),
        )
        .unwrap();

        assert_eq!(result.fetched, 1);
        assert_eq!(result.new, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.errors, 0);

        // Verify page is in index
        assert!(repo.notion_page_exists("page-abc").unwrap());
        // Verify cursor was updated
        assert!(repo.get_notion_sync_cursor("x_bookmark").unwrap().is_some());
    }

    #[test]
    fn notion_sync_skips_existing_page() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut engine = LlmWikiEngine::new(schema);

        // Pre-populate the page index
        let existing_source = wiki_core::SourceId(uuid::Uuid::new_v4());
        repo.insert_notion_page_index("page-existing", "x_bookmark", &existing_source)
            .unwrap();

        let pages = vec![make_page("page-existing", "Already Synced")];
        let result = run_sync_with_pages(
            "x_bookmark",
            pages,
            None,
            None,
            false,
            &repo,
            &mut engine,
            private_scope(),
        )
        .unwrap();

        assert_eq!(result.fetched, 1);
        assert_eq!(result.new, 0);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn notion_sync_dry_run_no_writes() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let schema = DomainSchema::permissive_default();
        let mut engine = LlmWikiEngine::new(schema);

        let pages = vec![
            make_page("page-1", "Page 1"),
            make_page("page-2", "Page 2"),
        ];
        let result = run_sync_with_pages(
            "x_bookmark",
            pages,
            None,
            None,
            true, // dry_run
            &repo,
            &mut engine,
            private_scope(),
        )
        .unwrap();

        assert_eq!(result.fetched, 2);
        assert_eq!(result.new, 2); // counts what would be new
        assert_eq!(result.skipped, 0);

        // No pages written to DB
        assert!(!repo.notion_page_exists("page-1").unwrap());
        assert!(!repo.notion_page_exists("page-2").unwrap());
        // No cursor updated
        assert!(repo.get_notion_sync_cursor("x_bookmark").unwrap().is_none());
    }

    #[test]
    fn notion_sync_body_format() {
        let page = NotionPage {
            id: "p".to_string(),
            last_edited_time: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            title: "My Title".to_string(),
            url: Some("https://example.com".to_string()),
            tags: vec![],
            source: Some("X".to_string()),
            note: Some("a note".to_string()),
            status: Some("待读".to_string()),
        };
        let body = build_body(&page);
        assert!(body.starts_with("# My Title"));
        assert!(body.contains("URL: https://example.com"));
        assert!(body.contains("来源: X"));
        assert!(body.contains("状态: 待读"));
        assert!(body.contains("备注: a note"));
    }

    #[test]
    fn notion_sync_body_omits_empty_fields() {
        let page = NotionPage {
            id: "p".to_string(),
            last_edited_time: OffsetDateTime::from_unix_timestamp(0).unwrap(),
            title: "Title Only".to_string(),
            url: None,
            tags: vec![],
            source: None,
            note: None,
            status: None,
        };
        let body = build_body(&page);
        assert!(body.starts_with("# Title Only"));
        assert!(!body.contains("URL:"));
        assert!(!body.contains("来源:"));
        assert!(!body.contains("备注:"));
    }
}
