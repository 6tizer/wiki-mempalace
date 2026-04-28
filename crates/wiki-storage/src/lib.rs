use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use time::{
    format_description, format_description::well_known::Rfc3339, OffsetDateTime, PrimitiveDateTime,
};
use wiki_core::{
    AuditRecord, Claim, Entity, EntryType, PageId, RawArtifact, Scope, SourceId, TypedEdge,
    WikiEvent, WikiPage,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageSnapshot {
    pub sources: Vec<RawArtifact>,
    pub claims: Vec<Claim>,
    pub pages: Vec<WikiPage>,
    pub entities: Vec<Entity>,
    pub edges: Vec<TypedEdge>,
    pub audits: Vec<AuditRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutomationRunStatus {
    Running,
    Succeeded,
    Failed,
}

impl AutomationRunStatus {
    fn as_str(self) -> &'static str {
        match self {
            AutomationRunStatus::Running => "running",
            AutomationRunStatus::Succeeded => "succeeded",
            AutomationRunStatus::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, StorageError> {
        match value {
            "running" => Ok(AutomationRunStatus::Running),
            "succeeded" => Ok(AutomationRunStatus::Succeeded),
            "failed" => Ok(AutomationRunStatus::Failed),
            other => Err(StorageError::InvalidAutomationRunState(format!(
                "unknown status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRunRecord {
    pub id: i64,
    pub job_name: String,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub status: AutomationRunStatus,
    pub duration_ms: Option<i64>,
    pub error_summary: Option<String>,
    pub heartbeat_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxStats {
    pub head_id: i64,
    pub total_events: i64,
    pub unprocessed_events: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxConsumerProgress {
    pub consumer_tag: String,
    pub acked_up_to_id: Option<i64>,
    pub acked_at: Option<OffsetDateTime>,
    pub backlog_events: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxConsumerCursorExport {
    pub consumer_tag: String,
    pub start_after_id: i64,
    pub head_id: i64,
    pub event_count: usize,
    pub ndjson: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationJobFailureSummary {
    pub job_name: String,
    pub consecutive_failures: usize,
    pub latest_failure: Option<AutomationRunRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalAliasMapping {
    pub alias_text: String,
    pub normalized_alias_key: String,
    pub canonical_page_id: PageId,
    pub canonical_title: String,
    pub entry_type: EntryType,
    pub scope: Scope,
    pub source: String,
    pub confidence: f64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub fn canonical_notion_page_id(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|ch| *ch != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

pub trait WikiRepository {
    fn load_snapshot(&self) -> Result<StorageSnapshot, StorageError>;
    fn save_snapshot(&self, snapshot: &StorageSnapshot) -> Result<(), StorageError>;
    fn append_outbox(&self, event: &WikiEvent) -> Result<(), StorageError>;
    fn append_outbox_batch(&self, events: &[WikiEvent]) -> Result<usize, StorageError> {
        for event in events {
            self.append_outbox(event)?;
        }
        Ok(events.len())
    }
    fn save_snapshot_and_append_outbox(
        &self,
        snapshot: &StorageSnapshot,
        events: &[WikiEvent],
    ) -> Result<usize, StorageError>;
    fn export_outbox_ndjson(&self) -> Result<String, StorageError>;
    fn export_outbox_ndjson_from_id(&self, last_id: i64) -> Result<String, StorageError>;
    fn export_outbox_ndjson_for_consumer(
        &self,
        consumer_tag: &str,
    ) -> Result<OutboxConsumerCursorExport, StorageError>;
    fn mark_outbox_processed(
        &self,
        up_to_id: i64,
        consumer_tag: &str,
    ) -> Result<usize, StorageError>;

    // --- Notion incremental sync ---

    fn get_notion_sync_cursor(&self, db_id: &str) -> Result<Option<OffsetDateTime>, StorageError>;

    fn upsert_notion_sync_cursor(
        &self,
        db_id: &str,
        at: OffsetDateTime,
        pages_synced_increment: i64,
    ) -> Result<(), StorageError>;

    fn notion_page_exists(&self, notion_page_id: &str) -> Result<bool, StorageError>;

    fn insert_notion_page_index(
        &self,
        notion_page_id: &str,
        db_id: &str,
        source_id: &SourceId,
    ) -> Result<(), StorageError>;

    fn insert_notion_page_indexes(
        &self,
        entries: &[(String, String, SourceId)],
    ) -> Result<(), StorageError>;

    // --- Compiler canonicalization aliases ---

    fn upsert_canonical_alias(&self, mapping: &CanonicalAliasMapping) -> Result<(), StorageError>;

    fn find_canonical_alias(
        &self,
        scope: &Scope,
        normalized_alias_key: &str,
    ) -> Result<Option<CanonicalAliasMapping>, StorageError>;

    fn list_canonical_aliases_for_pages(
        &self,
        page_ids: &[PageId],
    ) -> Result<Vec<CanonicalAliasMapping>, StorageError>;

    fn list_canonical_aliases_for_scope(
        &self,
        scope: &Scope,
    ) -> Result<Vec<CanonicalAliasMapping>, StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("db: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid automation run state: {0}")]
    InvalidAutomationRunState(String),
    #[error("automation run not found: {0}")]
    NotFound(String),
    #[error("invalid canonical alias: {0}")]
    InvalidCanonicalAlias(String),
}

pub struct SqliteRepository {
    conn: Connection,
}

impl SqliteRepository {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS wiki_state (
  id INTEGER PRIMARY KEY CHECK (id=1),
  payload_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS wiki_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  processed_at TEXT,
  consumer_tag TEXT
);
CREATE TABLE IF NOT EXISTS wiki_outbox_consumer_progress (
  consumer_tag TEXT PRIMARY KEY,
  acked_up_to_id INTEGER NOT NULL,
  acked_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS wiki_embedding (
  doc_id TEXT PRIMARY KEY,
  dim INTEGER NOT NULL,
  vec BLOB NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE IF NOT EXISTS wiki_automation_run (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_name TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  status TEXT NOT NULL,
  duration_ms INTEGER,
  error_summary TEXT,
  heartbeat_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS wiki_automation_run_job_id_idx
  ON wiki_automation_run(job_name, id DESC);
CREATE INDEX IF NOT EXISTS wiki_automation_run_job_status_id_idx
  ON wiki_automation_run(job_name, status, id DESC);
CREATE TABLE IF NOT EXISTS notion_sync_cursors (
  db_id TEXT PRIMARY KEY,
  last_synced_at TEXT NOT NULL,
  pages_synced INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS notion_page_index (
  notion_page_id TEXT PRIMARY KEY,
  db_id TEXT NOT NULL,
  source_id TEXT NOT NULL,
  synced_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS wiki_canonical_alias (
  alias_text TEXT NOT NULL,
  normalized_alias_key TEXT NOT NULL,
  canonical_page_id TEXT NOT NULL,
  canonical_title TEXT NOT NULL,
  entry_type TEXT NOT NULL,
  scope TEXT NOT NULL,
  source TEXT NOT NULL,
  confidence REAL NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(scope, normalized_alias_key)
);
CREATE INDEX IF NOT EXISTS wiki_canonical_alias_page_idx
  ON wiki_canonical_alias(canonical_page_id);
"#,
        )?;
        Ok(Self { conn })
    }

    pub fn start_automation_run(&self, job_name: &str) -> Result<i64, StorageError> {
        self.start_automation_run_at(job_name, OffsetDateTime::now_utc())
    }

    pub fn refresh_automation_heartbeat(&self, run_id: i64) -> Result<(), StorageError> {
        self.refresh_automation_heartbeat_at(run_id, OffsetDateTime::now_utc())
    }

    pub fn mark_automation_run_succeeded(&self, run_id: i64) -> Result<(), StorageError> {
        self.mark_automation_run_succeeded_at(run_id, OffsetDateTime::now_utc())
    }

    pub fn mark_automation_run_failed(
        &self,
        run_id: i64,
        error_summary: &str,
    ) -> Result<(), StorageError> {
        self.mark_automation_run_failed_at(run_id, OffsetDateTime::now_utc(), error_summary)
    }

    pub fn get_latest_automation_run(
        &self,
        job_name: &str,
    ) -> Result<Option<AutomationRunRecord>, StorageError> {
        self.query_latest_automation_run(job_name, None)
    }

    pub fn get_latest_successful_automation_run(
        &self,
        job_name: &str,
    ) -> Result<Option<AutomationRunRecord>, StorageError> {
        self.query_latest_automation_run(job_name, Some(AutomationRunStatus::Succeeded))
    }

    pub fn list_recent_failed_automation_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomationRunRecord>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, job_name, started_at, finished_at, status, duration_ms, error_summary, heartbeat_at
             FROM wiki_automation_run
             WHERE status = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(
            params![AutomationRunStatus::Failed.as_str(), limit as i64],
            decode_automation_run_row,
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn count_consecutive_automation_run_failures(
        &self,
        job_name: &str,
    ) -> Result<usize, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT status
             FROM wiki_automation_run
             WHERE job_name = ?1
             ORDER BY id DESC
             LIMIT 64",
        )?;
        let mut rows = stmt.query(params![job_name])?;
        let mut count = 0usize;
        while let Some(row) = rows.next()? {
            let status_raw: String = row.get(0)?;
            let status = AutomationRunStatus::parse(&status_raw)?;
            if status == AutomationRunStatus::Failed {
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }

    pub fn list_automation_job_failure_summaries(
        &self,
    ) -> Result<Vec<AutomationJobFailureSummary>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT job_name
             FROM wiki_automation_run
             ORDER BY job_name ASC",
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let job_name: String = row.get(0)?;
            let consecutive_failures = self.count_consecutive_automation_run_failures(&job_name)?;
            if consecutive_failures == 0 {
                continue;
            }
            out.push(AutomationJobFailureSummary {
                latest_failure: self
                    .query_latest_automation_run(&job_name, Some(AutomationRunStatus::Failed))?,
                job_name,
                consecutive_failures,
            });
        }
        out.sort_by(|a, b| {
            b.consecutive_failures
                .cmp(&a.consecutive_failures)
                .then_with(|| a.job_name.cmp(&b.job_name))
        });
        Ok(out)
    }

    pub fn get_outbox_stats(&self) -> Result<OutboxStats, StorageError> {
        let (head_id, total_events, unprocessed_events): (i64, i64, i64) = self.conn.query_row(
            "SELECT
                COALESCE(MAX(id), 0),
                COUNT(*),
                COALESCE(SUM(CASE WHEN processed_at IS NULL THEN 1 ELSE 0 END), 0)
             FROM wiki_outbox",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(OutboxStats {
            head_id,
            total_events,
            unprocessed_events,
        })
    }

    pub fn integrity_check(&self) -> Result<String, StorageError> {
        let integrity: String = self
            .conn
            .query_row("PRAGMA integrity_check;", [], |row| row.get(0))?;
        Ok(integrity)
    }

    pub fn get_outbox_consumer_progress(
        &self,
        consumer_tag: &str,
    ) -> Result<OutboxConsumerProgress, StorageError> {
        let stats = self.get_outbox_stats()?;
        let row = self.conn.query_row(
            "SELECT acked_up_to_id, acked_at
             FROM wiki_outbox_consumer_progress
             WHERE consumer_tag = ?1",
            params![consumer_tag],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        );
        let (acked_up_to_id, acked_at) = match row {
            Ok((id, acked_at_raw)) => (Some(id), Some(parse_time(&acked_at_raw)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => (None, None),
            Err(e) => return Err(StorageError::Db(e)),
        };
        let backlog_events = match acked_up_to_id {
            Some(id) => stats.head_id.saturating_sub(id),
            None => stats.head_id,
        };
        Ok(OutboxConsumerProgress {
            consumer_tag: consumer_tag.to_string(),
            acked_up_to_id,
            acked_at,
            backlog_events,
        })
    }

    /// 写入或更新一条向量（`vec` 为 little-endian `f32` 序列）。
    pub fn upsert_embedding(&self, doc_id: &str, vector: &[f32]) -> Result<(), StorageError> {
        let dim = vector.len() as i32;
        let mut blob = Vec::with_capacity(vector.len() * 4);
        for x in vector {
            blob.extend_from_slice(&x.to_le_bytes());
        }
        self.conn.execute(
            "INSERT INTO wiki_embedding(doc_id, dim, vec, updated_at)
             VALUES(?1, ?2, ?3, datetime('now'))
             ON CONFLICT(doc_id) DO UPDATE SET dim=excluded.dim, vec=excluded.vec, updated_at=excluded.updated_at",
            params![doc_id, dim, blob],
        )?;
        Ok(())
    }

    pub fn delete_embedding(&self, doc_id: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "DELETE FROM wiki_embedding WHERE doc_id = ?1",
            params![doc_id],
        )?;
        Ok(())
    }

    /// 与 `query` 同维度的行做 cosine 相似度，返回 `(doc_id, score)` 降序。
    pub fn search_embeddings_cosine(
        &self,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<(String, f32)>, StorageError> {
        let qn = l2_norm(query);
        if qn <= 1e-12 || limit == 0 {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .conn
            .prepare("SELECT doc_id, dim, vec FROM wiki_embedding")?;
        let mut rows = stmt.query([])?;
        let mut scored: Vec<(String, f32)> = Vec::new();
        while let Some(r) = rows.next()? {
            let doc_id: String = r.get(0)?;
            let dim: i32 = r.get(1)?;
            let blob: Vec<u8> = r.get(2)?;
            let Some(v) = try_blob_to_f32(&blob, dim as usize) else {
                eprintln!(
                    "warning: wiki_embedding row doc_id={doc_id} blob length mismatch (expected {} bytes, got {})",
                    dim as usize * 4,
                    blob.len()
                );
                continue;
            };
            if v.len() != query.len() {
                eprintln!(
                    "warning: wiki_embedding row doc_id={doc_id} dim mismatch (expected {}, got {})",
                    query.len(),
                    v.len()
                );
                continue;
            }
            let vn = l2_norm(&v);
            if vn <= 1e-12 {
                continue;
            }
            let dot: f32 = query.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
            let c = dot / (qn * vn);
            if c.is_nan() {
                continue;
            }
            scored.push((doc_id, c));
        }
        scored.sort_by(|a, b| a.1.total_cmp(&b.1).reverse().then_with(|| a.0.cmp(&b.0)));
        scored.truncate(limit);
        Ok(scored)
    }

    pub fn upsert_canonical_alias(
        &self,
        mapping: &CanonicalAliasMapping,
    ) -> Result<(), StorageError> {
        <Self as WikiRepository>::upsert_canonical_alias(self, mapping)
    }

    pub fn find_canonical_alias(
        &self,
        scope: &Scope,
        normalized_alias_key: &str,
    ) -> Result<Option<CanonicalAliasMapping>, StorageError> {
        <Self as WikiRepository>::find_canonical_alias(self, scope, normalized_alias_key)
    }

    pub fn list_canonical_aliases_for_pages(
        &self,
        page_ids: &[PageId],
    ) -> Result<Vec<CanonicalAliasMapping>, StorageError> {
        <Self as WikiRepository>::list_canonical_aliases_for_pages(self, page_ids)
    }

    pub fn list_canonical_aliases_for_scope(
        &self,
        scope: &Scope,
    ) -> Result<Vec<CanonicalAliasMapping>, StorageError> {
        <Self as WikiRepository>::list_canonical_aliases_for_scope(self, scope)
    }

    pub fn save_snapshot_and_append_outbox_with_aliases(
        &self,
        snapshot: &StorageSnapshot,
        events: &[WikiEvent],
        aliases: &[CanonicalAliasMapping],
    ) -> Result<usize, StorageError> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            let n = self.save_snapshot_and_append_outbox_inner(snapshot, events)?;
            for alias in aliases {
                self.upsert_canonical_alias_inner(alias)?;
            }
            Ok(n)
        })();
        match result {
            Ok(n) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(n)
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn start_automation_run_at(
        &self,
        job_name: &str,
        started_at: OffsetDateTime,
    ) -> Result<i64, StorageError> {
        let started_at = encode_time(started_at)?;
        self.conn.execute(
            "INSERT INTO wiki_automation_run(job_name, started_at, status, heartbeat_at)
             VALUES(?1, ?2, ?3, ?4)",
            params![
                job_name,
                started_at,
                AutomationRunStatus::Running.as_str(),
                started_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn refresh_automation_heartbeat_at(
        &self,
        run_id: i64,
        heartbeat_at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        let heartbeat_at = encode_time(heartbeat_at)?;
        let updated = self.conn.execute(
            "UPDATE wiki_automation_run
             SET heartbeat_at = ?2
             WHERE id = ?1 AND finished_at IS NULL AND status = ?3",
            params![run_id, heartbeat_at, AutomationRunStatus::Running.as_str()],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!("run_id={run_id}")));
        }
        Ok(())
    }

    fn mark_automation_run_succeeded_at(
        &self,
        run_id: i64,
        finished_at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        self.finish_automation_run(run_id, finished_at, AutomationRunStatus::Succeeded, None)
    }

    fn mark_automation_run_failed_at(
        &self,
        run_id: i64,
        finished_at: OffsetDateTime,
        error_summary: &str,
    ) -> Result<(), StorageError> {
        self.finish_automation_run(
            run_id,
            finished_at,
            AutomationRunStatus::Failed,
            Some(error_summary),
        )
    }

    fn finish_automation_run(
        &self,
        run_id: i64,
        finished_at: OffsetDateTime,
        status: AutomationRunStatus,
        error_summary: Option<&str>,
    ) -> Result<(), StorageError> {
        let (started_at_raw, current_status): (String, String) = match self.conn.query_row(
            "SELECT started_at, status FROM wiki_automation_run WHERE id = ?1",
            params![run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(StorageError::NotFound(format!("run_id={run_id}")));
            }
            Err(e) => return Err(StorageError::Db(e)),
        };
        if current_status != AutomationRunStatus::Running.as_str() {
            return Err(StorageError::NotFound(format!("run_id={run_id}")));
        }
        let started_at = parse_time(&started_at_raw)?;
        let finished_at_raw = encode_time(finished_at)?;
        let duration_ms =
            i64::try_from((finished_at - started_at).whole_milliseconds()).map_err(|_| {
                StorageError::InvalidAutomationRunState(format!(
                    "duration overflow for run_id={run_id}"
                ))
            })?;
        let updated = self.conn.execute(
            "UPDATE wiki_automation_run
             SET finished_at = ?2,
                 status = ?3,
                 duration_ms = ?4,
                 error_summary = ?5,
                 heartbeat_at = ?2
             WHERE id = ?1 AND finished_at IS NULL AND status = ?6",
            params![
                run_id,
                finished_at_raw,
                status.as_str(),
                duration_ms,
                error_summary,
                AutomationRunStatus::Running.as_str()
            ],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!("run_id={run_id}")));
        }
        Ok(())
    }

    fn query_latest_automation_run(
        &self,
        job_name: &str,
        status: Option<AutomationRunStatus>,
    ) -> Result<Option<AutomationRunRecord>, StorageError> {
        let sql = match status {
            Some(_) => {
                "SELECT id, job_name, started_at, finished_at, status, duration_ms, error_summary, heartbeat_at
                 FROM wiki_automation_run
                 WHERE job_name = ?1 AND status = ?2
                 ORDER BY id DESC
                 LIMIT 1"
            }
            None => {
                "SELECT id, job_name, started_at, finished_at, status, duration_ms, error_summary, heartbeat_at
                 FROM wiki_automation_run
                 WHERE job_name = ?1
                 ORDER BY id DESC
                 LIMIT 1"
            }
        };
        let result = match status {
            Some(status) => self
                .conn
                .query_row(sql, params![job_name, status.as_str()], |row| {
                    decode_automation_run_row(row)
                }),
            None => self
                .conn
                .query_row(sql, params![job_name], decode_automation_run_row),
        };
        match result {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Db(e)),
        }
    }

    fn mark_outbox_processed_inner(
        &self,
        up_to_id: i64,
        consumer_tag: &str,
    ) -> Result<usize, StorageError> {
        let previous_ack = self
            .conn
            .query_row(
                "SELECT acked_up_to_id
                 FROM wiki_outbox_consumer_progress
                 WHERE consumer_tag = ?1",
                params![consumer_tag],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        if up_to_id <= previous_ack {
            return Ok(0);
        }

        let newly_acked: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM wiki_outbox
             WHERE id > ?1 AND id <= ?2 AND processed_at IS NULL",
            params![previous_ack, up_to_id],
            |row| row.get(0),
        )?;

        self.conn.execute(
            "INSERT INTO wiki_outbox_consumer_progress(consumer_tag, acked_up_to_id, acked_at)
             VALUES(?1, ?2, datetime('now'))
             ON CONFLICT(consumer_tag) DO UPDATE SET
               acked_up_to_id = excluded.acked_up_to_id,
               acked_at = excluded.acked_at",
            params![consumer_tag, up_to_id],
        )?;

        self.conn.execute(
            "UPDATE wiki_outbox
             SET processed_at = datetime('now'), consumer_tag = ?2
             WHERE id <= ?1 AND processed_at IS NULL",
            params![up_to_id, consumer_tag],
        )?;
        Ok(newly_acked as usize)
    }

    fn save_snapshot_and_append_outbox_inner(
        &self,
        snapshot: &StorageSnapshot,
        events: &[WikiEvent],
    ) -> Result<usize, StorageError> {
        let payload = serde_json::to_string(snapshot)?;
        self.conn.execute(
            "INSERT INTO wiki_state(id, payload_json) VALUES(1, ?1)
             ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json",
            params![payload],
        )?;
        for event in events {
            let payload = serde_json::to_string(event)?;
            self.conn.execute(
                "INSERT INTO wiki_outbox(event_json) VALUES(?1)",
                params![payload],
            )?;
        }
        Ok(events.len())
    }

    fn upsert_canonical_alias_inner(
        &self,
        mapping: &CanonicalAliasMapping,
    ) -> Result<(), StorageError> {
        let canonical_page_id = encode_page_id(&mapping.canonical_page_id)?;
        let entry_type = encode_entry_type(&mapping.entry_type)?;
        let scope = encode_scope(&mapping.scope)?;
        let created_at = encode_time(mapping.created_at)?;
        let updated_at = encode_time(mapping.updated_at)?;
        self.conn.execute(
            "INSERT INTO wiki_canonical_alias(
               alias_text,
               normalized_alias_key,
               canonical_page_id,
               canonical_title,
               entry_type,
               scope,
               source,
               confidence,
               created_at,
               updated_at
             )
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(scope, normalized_alias_key) DO UPDATE SET
               alias_text = excluded.alias_text,
               canonical_page_id = excluded.canonical_page_id,
               canonical_title = excluded.canonical_title,
               entry_type = excluded.entry_type,
               source = excluded.source,
               confidence = excluded.confidence,
               updated_at = excluded.updated_at",
            params![
                mapping.alias_text,
                mapping.normalized_alias_key,
                canonical_page_id,
                mapping.canonical_title,
                entry_type,
                scope,
                mapping.source,
                mapping.confidence,
                created_at,
                updated_at
            ],
        )?;
        Ok(())
    }
}

fn try_blob_to_f32(blob: &[u8], expected_len: usize) -> Option<Vec<f32>> {
    if blob.len() != expected_len * 4 {
        return None;
    }
    let mut out = Vec::with_capacity(expected_len);
    for chunk in blob.chunks_exact(4) {
        out.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Some(out)
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn encode_time(value: OffsetDateTime) -> Result<String, StorageError> {
    value
        .format(&Rfc3339)
        .map_err(|err| StorageError::InvalidAutomationRunState(format!("invalid timestamp: {err}")))
}

fn parse_time(value: &str) -> Result<OffsetDateTime, StorageError> {
    if let Ok(ts) = OffsetDateTime::parse(value, &Rfc3339) {
        return Ok(ts);
    }
    let sqlite_fmt = format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")
        .map_err(|err| {
            StorageError::InvalidAutomationRunState(format!(
                "invalid sqlite timestamp format description: {err}"
            ))
        })?;
    PrimitiveDateTime::parse(value, &sqlite_fmt)
        .map(|dt| dt.assume_utc())
        .map_err(|err| {
            StorageError::InvalidAutomationRunState(format!("invalid timestamp {value:?}: {err}"))
        })
}

fn decode_automation_run_row(
    row: &rusqlite::Row<'_>,
) -> Result<AutomationRunRecord, rusqlite::Error> {
    let id: i64 = row.get(0)?;
    let job_name: String = row.get(1)?;
    let started_at_raw: String = row.get(2)?;
    let finished_at_raw: Option<String> = row.get(3)?;
    let status_raw: String = row.get(4)?;
    let duration_ms: Option<i64> = row.get(5)?;
    let error_summary: Option<String> = row.get(6)?;
    let heartbeat_at_raw: String = row.get(7)?;

    let started_at = parse_time(&started_at_raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let finished_at = match finished_at_raw {
        Some(value) => Some(parse_time(&value).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err))
        })?),
        None => None,
    };
    let heartbeat_at = parse_time(&heartbeat_at_raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let status = AutomationRunStatus::parse(&status_raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(AutomationRunRecord {
        id,
        job_name,
        started_at,
        finished_at,
        status,
        duration_ms,
        error_summary,
        heartbeat_at,
    })
}

fn encode_scope(scope: &Scope) -> Result<String, StorageError> {
    Ok(serde_json::to_string(scope)?)
}

fn encode_entry_type(entry_type: &EntryType) -> Result<String, StorageError> {
    match serde_json::to_value(entry_type)? {
        serde_json::Value::String(value) => Ok(value),
        value => Err(StorageError::InvalidCanonicalAlias(format!(
            "entry_type encoded as non-string JSON: {value}"
        ))),
    }
}

fn encode_page_id(page_id: &PageId) -> Result<String, StorageError> {
    match serde_json::to_value(page_id)? {
        serde_json::Value::String(value) => Ok(value),
        value => Err(StorageError::InvalidCanonicalAlias(format!(
            "page id encoded as non-string JSON: {value}"
        ))),
    }
}

fn decode_canonical_alias_row(
    row: &rusqlite::Row<'_>,
) -> Result<CanonicalAliasMapping, rusqlite::Error> {
    let alias_text: String = row.get(0)?;
    let normalized_alias_key: String = row.get(1)?;
    let canonical_page_id_raw: String = row.get(2)?;
    let canonical_title: String = row.get(3)?;
    let entry_type_raw: String = row.get(4)?;
    let scope_raw: String = row.get(5)?;
    let source: String = row.get(6)?;
    let confidence: f64 = row.get(7)?;
    let created_at_raw: String = row.get(8)?;
    let updated_at_raw: String = row.get(9)?;

    let canonical_page_id = serde_json::from_value(serde_json::Value::String(
        canonical_page_id_raw,
    ))
    .map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let entry_type =
        serde_json::from_value(serde_json::Value::String(entry_type_raw)).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
        })?;
    let scope = serde_json::from_str(&scope_raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let created_at = parse_time(&created_at_raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let updated_at = parse_time(&updated_at_raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(CanonicalAliasMapping {
        alias_text,
        normalized_alias_key,
        canonical_page_id,
        canonical_title,
        entry_type,
        scope,
        source,
        confidence,
        created_at,
        updated_at,
    })
}

impl WikiRepository for SqliteRepository {
    fn load_snapshot(&self) -> Result<StorageSnapshot, StorageError> {
        let row = self
            .conn
            .query_row("SELECT payload_json FROM wiki_state WHERE id=1", [], |r| {
                r.get::<_, String>(0)
            });
        match row {
            Ok(payload) => Ok(serde_json::from_str(&payload)?),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(StorageSnapshot::default()),
            Err(e) => Err(StorageError::Db(e)),
        }
    }

    fn save_snapshot(&self, snapshot: &StorageSnapshot) -> Result<(), StorageError> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.save_snapshot_and_append_outbox_inner(snapshot, &[]);
        match result {
            Ok(_) => self.conn.execute_batch("COMMIT")?,
            Err(_) => {
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        result.map(|_| ())
    }

    fn append_outbox(&self, event: &WikiEvent) -> Result<(), StorageError> {
        let payload = serde_json::to_string(event)?;
        self.conn.execute(
            "INSERT INTO wiki_outbox(event_json) VALUES(?1)",
            params![payload],
        )?;
        Ok(())
    }

    fn append_outbox_batch(&self, events: &[WikiEvent]) -> Result<usize, StorageError> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            for event in events {
                let payload = serde_json::to_string(event)?;
                self.conn.execute(
                    "INSERT INTO wiki_outbox(event_json) VALUES(?1)",
                    params![payload],
                )?;
            }
            Ok(events.len())
        })();
        match result {
            Ok(_) => self.conn.execute_batch("COMMIT")?,
            Err(_) => {
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        result
    }

    fn save_snapshot_and_append_outbox(
        &self,
        snapshot: &StorageSnapshot,
        events: &[WikiEvent],
    ) -> Result<usize, StorageError> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.save_snapshot_and_append_outbox_inner(snapshot, events);
        match result {
            Ok(_) => self.conn.execute_batch("COMMIT")?,
            Err(_) => {
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        result
    }

    fn export_outbox_ndjson(&self) -> Result<String, StorageError> {
        self.export_outbox_ndjson_from_id(0)
    }

    fn export_outbox_ndjson_from_id(&self, last_id: i64) -> Result<String, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT event_json FROM wiki_outbox WHERE id > ?1 ORDER BY id ASC")?;
        let mut out = String::new();
        let mut rows = stmt.query(params![last_id])?;
        while let Some(r) = rows.next()? {
            let line: String = r.get(0)?;
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out)
    }

    fn export_outbox_ndjson_for_consumer(
        &self,
        consumer_tag: &str,
    ) -> Result<OutboxConsumerCursorExport, StorageError> {
        let progress = self.get_outbox_consumer_progress(consumer_tag)?;
        let start_after_id = progress.acked_up_to_id.unwrap_or(0);
        let stats = self.get_outbox_stats()?;
        let ndjson = self.export_outbox_ndjson_from_id(start_after_id)?;
        let event_count = ndjson.lines().count();
        Ok(OutboxConsumerCursorExport {
            consumer_tag: consumer_tag.to_string(),
            start_after_id,
            head_id: stats.head_id,
            event_count,
            ndjson,
        })
    }

    fn mark_outbox_processed(
        &self,
        up_to_id: i64,
        consumer_tag: &str,
    ) -> Result<usize, StorageError> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.mark_outbox_processed_inner(up_to_id, consumer_tag);
        match result {
            Ok(_) => self.conn.execute_batch("COMMIT")?,
            Err(_) => {
                let _ = self.conn.execute_batch("ROLLBACK");
            }
        }
        result
    }

    fn get_notion_sync_cursor(&self, db_id: &str) -> Result<Option<OffsetDateTime>, StorageError> {
        let result = self
            .conn
            .query_row(
                "SELECT last_synced_at FROM notion_sync_cursors WHERE db_id = ?1",
                params![db_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match result {
            Some(value) => Ok(Some(parse_time(&value)?)),
            None => Ok(None),
        }
    }

    fn upsert_notion_sync_cursor(
        &self,
        db_id: &str,
        at: OffsetDateTime,
        pages_synced_increment: i64,
    ) -> Result<(), StorageError> {
        let at_str = encode_time(at)?;
        self.conn.execute(
            "INSERT INTO notion_sync_cursors(db_id, last_synced_at, pages_synced)
             VALUES(?1, ?2, ?3)
             ON CONFLICT(db_id) DO UPDATE SET
               last_synced_at = excluded.last_synced_at,
               pages_synced = pages_synced + excluded.pages_synced",
            params![db_id, at_str, pages_synced_increment],
        )?;
        Ok(())
    }

    fn notion_page_exists(&self, notion_page_id: &str) -> Result<bool, StorageError> {
        let notion_page_id = canonical_notion_page_id(notion_page_id);
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM notion_page_index WHERE notion_page_id = ?1",
            params![notion_page_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn insert_notion_page_index(
        &self,
        notion_page_id: &str,
        db_id: &str,
        source_id: &SourceId,
    ) -> Result<(), StorageError> {
        let notion_page_id = canonical_notion_page_id(notion_page_id);
        let now_str = encode_time(OffsetDateTime::now_utc())?;
        self.conn.execute(
            "INSERT OR IGNORE INTO notion_page_index(notion_page_id, db_id, source_id, synced_at)
             VALUES(?1, ?2, ?3, ?4)",
            params![notion_page_id, db_id, source_id.0.to_string(), now_str],
        )?;
        Ok(())
    }

    fn insert_notion_page_indexes(
        &self,
        entries: &[(String, String, SourceId)],
    ) -> Result<(), StorageError> {
        let now_str = encode_time(OffsetDateTime::now_utc())?;
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = {
            let mut stmt = self.conn.prepare(
                "INSERT OR IGNORE INTO notion_page_index(notion_page_id, db_id, source_id, synced_at)
                 VALUES(?1, ?2, ?3, ?4)",
            )?;
            for (notion_page_id, db_id, source_id) in entries {
                let notion_page_id = canonical_notion_page_id(notion_page_id);
                stmt.execute(params![
                    notion_page_id,
                    db_id,
                    source_id.0.to_string(),
                    now_str.clone()
                ])?;
            }
            Ok::<(), StorageError>(())
        };
        match result {
            Ok(_) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn upsert_canonical_alias(&self, mapping: &CanonicalAliasMapping) -> Result<(), StorageError> {
        self.upsert_canonical_alias_inner(mapping)
    }

    fn find_canonical_alias(
        &self,
        scope: &Scope,
        normalized_alias_key: &str,
    ) -> Result<Option<CanonicalAliasMapping>, StorageError> {
        let scope = encode_scope(scope)?;
        let result = self
            .conn
            .query_row(
                "SELECT
                   alias_text,
                   normalized_alias_key,
                   canonical_page_id,
                   canonical_title,
                   entry_type,
                   scope,
                   source,
                   confidence,
                   created_at,
                   updated_at
                 FROM wiki_canonical_alias
                 WHERE scope = ?1 AND normalized_alias_key = ?2",
                params![scope, normalized_alias_key],
                decode_canonical_alias_row,
            )
            .optional()?;
        Ok(result)
    }

    fn list_canonical_aliases_for_pages(
        &self,
        page_ids: &[PageId],
    ) -> Result<Vec<CanonicalAliasMapping>, StorageError> {
        let mut out = Vec::new();
        let mut stmt = self.conn.prepare(
            "SELECT
               alias_text,
               normalized_alias_key,
               canonical_page_id,
               canonical_title,
               entry_type,
               scope,
               source,
               confidence,
               created_at,
               updated_at
             FROM wiki_canonical_alias
             WHERE canonical_page_id = ?1
             ORDER BY scope ASC, normalized_alias_key ASC",
        )?;
        for page_id in page_ids {
            let page_id = encode_page_id(page_id)?;
            let rows = stmt.query_map(params![page_id], decode_canonical_alias_row)?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    fn list_canonical_aliases_for_scope(
        &self,
        scope: &Scope,
    ) -> Result<Vec<CanonicalAliasMapping>, StorageError> {
        let scope = encode_scope(scope)?;
        let mut stmt = self.conn.prepare(
            "SELECT
               alias_text,
               normalized_alias_key,
               canonical_page_id,
               canonical_title,
               entry_type,
               scope,
               source,
               confidence,
               created_at,
               updated_at
             FROM wiki_canonical_alias
             WHERE scope = ?1
             ORDER BY normalized_alias_key ASC",
        )?;
        let rows = stmt.query_map(params![scope], decode_canonical_alias_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use wiki_core::{Scope, WikiEvent};

    #[test]
    fn outbox_export_from_id_and_ack() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        repo.append_outbox(&WikiEvent::QueryServed {
            query_fingerprint: "q1".into(),
            top_doc_ids: vec!["a".into()],
            at: time::OffsetDateTime::now_utc(),
        })
        .unwrap();
        repo.append_outbox(&WikiEvent::SourceIngested {
            source_id: wiki_core::SourceId(uuid::Uuid::new_v4()),
            redacted: false,
            at: time::OffsetDateTime::now_utc(),
        })
        .unwrap();

        let all = repo.export_outbox_ndjson().unwrap();
        assert!(all.lines().count() >= 2);

        let from1 = repo.export_outbox_ndjson_from_id(1).unwrap();
        assert!(from1.lines().count() >= 1);

        let acked = repo.mark_outbox_processed(1, "t").unwrap();
        assert_eq!(acked, 1);

        // Second ack should not re-ack already processed.
        let acked2 = repo.mark_outbox_processed(1, "t").unwrap();
        assert_eq!(acked2, 0);

        // Make sure schema stays loadable even with extra columns.
        let _snap = repo.load_snapshot().unwrap();
        let _scope = Scope::Private {
            agent_id: "a".into(),
        };
    }

    #[test]
    fn outbox_export_for_consumer_uses_independent_cursor() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        repo.append_outbox(&WikiEvent::QueryServed {
            query_fingerprint: "q1".into(),
            top_doc_ids: vec!["a".into()],
            at: time::OffsetDateTime::now_utc(),
        })
        .unwrap();
        repo.append_outbox(&WikiEvent::SourceIngested {
            source_id: wiki_core::SourceId(uuid::Uuid::new_v4()),
            redacted: false,
            at: time::OffsetDateTime::now_utc(),
        })
        .unwrap();

        let first = repo
            .export_outbox_ndjson_for_consumer("consumer-a")
            .unwrap();
        assert_eq!(first.consumer_tag, "consumer-a");
        assert_eq!(first.start_after_id, 0);
        assert_eq!(first.head_id, 2);
        assert_eq!(first.event_count, 2);

        repo.mark_outbox_processed(1, "consumer-a").unwrap();
        let after_ack = repo
            .export_outbox_ndjson_for_consumer("consumer-a")
            .unwrap();
        assert_eq!(after_ack.start_after_id, 1);
        assert_eq!(after_ack.event_count, 1);

        let other = repo
            .export_outbox_ndjson_for_consumer("consumer-b")
            .unwrap();
        assert_eq!(other.start_after_id, 0);
        assert_eq!(other.event_count, 2);
    }

    #[test]
    fn embedding_cosine_ranking() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        let c = vec![0.99_f32, 0.01, 0.0];
        repo.upsert_embedding("doc:a", &a).unwrap();
        repo.upsert_embedding("doc:b", &b).unwrap();
        repo.upsert_embedding("doc:c", &c).unwrap();
        let q = vec![1.0_f32, 0.0, 0.0];
        let hits = repo.search_embeddings_cosine(&q, 10).unwrap();
        assert_eq!(hits[0].0, "doc:a");
        assert!(hits[0].1 > hits[2].1);
    }

    #[test]
    fn automation_run_success_and_heartbeat_roundtrip() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let heartbeat = OffsetDateTime::from_unix_timestamp(1_700_000_060).unwrap();
        let finished = OffsetDateTime::from_unix_timestamp(1_700_000_120).unwrap();

        let run_id = repo.start_automation_run_at("batch-sync", start).unwrap();
        repo.refresh_automation_heartbeat_at(run_id, heartbeat)
            .unwrap();
        repo.mark_automation_run_succeeded_at(run_id, finished)
            .unwrap();

        let latest = repo
            .get_latest_automation_run("batch-sync")
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, run_id);
        assert_eq!(latest.job_name, "batch-sync");
        assert_eq!(latest.status, AutomationRunStatus::Succeeded);
        assert_eq!(latest.duration_ms, Some(120_000));
        assert_eq!(latest.finished_at, Some(finished));
        assert_eq!(latest.heartbeat_at, finished);

        let success = repo
            .get_latest_successful_automation_run("batch-sync")
            .unwrap()
            .unwrap();
        assert_eq!(success.id, run_id);
        assert_eq!(success.status, AutomationRunStatus::Succeeded);
    }

    #[test]
    fn automation_run_failure_and_latest_success() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let start_ok = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let finish_ok = OffsetDateTime::from_unix_timestamp(1_700_000_030).unwrap();
        let start_fail = OffsetDateTime::from_unix_timestamp(1_700_000_100).unwrap();
        let finish_fail = OffsetDateTime::from_unix_timestamp(1_700_000_150).unwrap();

        let ok_id = repo
            .start_automation_run_at("batch-sync", start_ok)
            .unwrap();
        repo.mark_automation_run_succeeded_at(ok_id, finish_ok)
            .unwrap();

        let fail_id = repo
            .start_automation_run_at("batch-sync", start_fail)
            .unwrap();
        repo.mark_automation_run_failed_at(fail_id, finish_fail, "network timeout")
            .unwrap();

        let latest = repo
            .get_latest_automation_run("batch-sync")
            .unwrap()
            .unwrap();
        assert_eq!(latest.id, fail_id);
        assert_eq!(latest.status, AutomationRunStatus::Failed);
        assert_eq!(latest.error_summary.as_deref(), Some("network timeout"));
        assert_eq!(latest.duration_ms, Some(50_000));

        let success = repo
            .get_latest_successful_automation_run("batch-sync")
            .unwrap()
            .unwrap();
        assert_eq!(success.id, ok_id);
        assert_eq!(success.status, AutomationRunStatus::Succeeded);
    }

    #[test]
    fn outbox_stats_empty_db_are_zero() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        let stats = repo.get_outbox_stats().unwrap();
        assert_eq!(
            stats,
            OutboxStats {
                head_id: 0,
                total_events: 0,
                unprocessed_events: 0
            }
        );

        let progress = repo.get_outbox_consumer_progress("mempalace").unwrap();
        assert_eq!(
            progress,
            OutboxConsumerProgress {
                consumer_tag: "mempalace".into(),
                acked_up_to_id: None,
                acked_at: None,
                backlog_events: 0
            }
        );
    }

    #[test]
    fn outbox_stats_and_consumer_progress_track_ack_and_backlog() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        for idx in 1..=3 {
            repo.append_outbox(&WikiEvent::QueryServed {
                query_fingerprint: format!("q{idx}"),
                top_doc_ids: vec![format!("doc:{idx}")],
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        }

        let stats_before_ack = repo.get_outbox_stats().unwrap();
        assert_eq!(stats_before_ack.head_id, 3);
        assert_eq!(stats_before_ack.total_events, 3);
        assert_eq!(stats_before_ack.unprocessed_events, 3);

        let progress_before_ack = repo.get_outbox_consumer_progress("mempalace").unwrap();
        assert_eq!(progress_before_ack.acked_up_to_id, None);
        assert_eq!(progress_before_ack.backlog_events, 3);

        repo.mark_outbox_processed(2, "mempalace").unwrap();

        let stats_after_ack = repo.get_outbox_stats().unwrap();
        assert_eq!(stats_after_ack.head_id, 3);
        assert_eq!(stats_after_ack.total_events, 3);
        assert_eq!(stats_after_ack.unprocessed_events, 1);

        let progress_after_ack = repo.get_outbox_consumer_progress("mempalace").unwrap();
        assert_eq!(progress_after_ack.acked_up_to_id, Some(2));
        assert!(progress_after_ack.acked_at.is_some());
        assert_eq!(progress_after_ack.backlog_events, 1);
    }

    #[test]
    fn outbox_consumer_progress_tracks_consumers_independently() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        for idx in 1..=4 {
            repo.append_outbox(&WikiEvent::QueryServed {
                query_fingerprint: format!("q{idx}"),
                top_doc_ids: vec![format!("doc:{idx}")],
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap();
        }

        assert_eq!(repo.mark_outbox_processed(2, "mempalace").unwrap(), 2);
        assert_eq!(repo.mark_outbox_processed(3, "archive").unwrap(), 1);

        let mempalace = repo.get_outbox_consumer_progress("mempalace").unwrap();
        let archive = repo.get_outbox_consumer_progress("archive").unwrap();
        assert_eq!(mempalace.acked_up_to_id, Some(2));
        assert_eq!(mempalace.backlog_events, 2);
        assert_eq!(archive.acked_up_to_id, Some(3));
        assert_eq!(archive.backlog_events, 1);

        assert_eq!(repo.mark_outbox_processed(4, "mempalace").unwrap(), 1);
        assert_eq!(repo.mark_outbox_processed(3, "archive").unwrap(), 0);

        let mempalace = repo.get_outbox_consumer_progress("mempalace").unwrap();
        let archive = repo.get_outbox_consumer_progress("archive").unwrap();
        assert_eq!(mempalace.acked_up_to_id, Some(4));
        assert_eq!(mempalace.backlog_events, 0);
        assert_eq!(archive.acked_up_to_id, Some(3));
        assert_eq!(archive.backlog_events, 1);
    }

    #[test]
    fn snapshot_and_outbox_commit_in_one_transaction() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let scope = Scope::Private {
            agent_id: "cli".into(),
        };
        let source = RawArtifact::new("file:///a.md", "alpha", scope);
        let source_id = source.id;
        let snapshot = StorageSnapshot {
            sources: vec![source],
            ..StorageSnapshot::default()
        };
        let event = WikiEvent::SourceIngested {
            source_id,
            redacted: false,
            at: OffsetDateTime::now_utc(),
        };

        let inserted = repo
            .save_snapshot_and_append_outbox(&snapshot, &[event])
            .unwrap();

        assert_eq!(inserted, 1);
        assert_eq!(repo.load_snapshot().unwrap().sources.len(), 1);
        assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 1);
    }

    #[test]
    fn snapshot_rolls_back_when_outbox_insert_fails() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let scope = Scope::Private {
            agent_id: "cli".into(),
        };
        let old_snapshot = StorageSnapshot {
            sources: vec![RawArtifact::new("file:///old.md", "old", scope.clone())],
            ..StorageSnapshot::default()
        };
        repo.save_snapshot(&old_snapshot).unwrap();
        repo.conn
            .execute_batch(
                "CREATE TRIGGER fail_outbox_insert
                 BEFORE INSERT ON wiki_outbox
                 BEGIN
                   SELECT RAISE(FAIL, 'forced outbox failure');
                 END;",
            )
            .unwrap();
        let new_source = RawArtifact::new("file:///new.md", "new", scope);
        let new_source_id = new_source.id;
        let new_snapshot = StorageSnapshot {
            sources: vec![new_source],
            ..StorageSnapshot::default()
        };
        let event = WikiEvent::SourceIngested {
            source_id: new_source_id,
            redacted: false,
            at: OffsetDateTime::now_utc(),
        };

        let err = repo
            .save_snapshot_and_append_outbox(&new_snapshot, &[event])
            .unwrap_err();

        assert!(format!("{err}").contains("forced outbox failure"));
        let restored = repo.load_snapshot().unwrap();
        assert_eq!(restored.sources.len(), 1);
        assert_eq!(restored.sources[0].uri, "file:///old.md");
        assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 0);
    }

    #[test]
    fn recent_failed_runs_and_consecutive_failures_are_reported() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        let lint_ok = repo
            .start_automation_run_at(
                "lint",
                OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            )
            .unwrap();
        repo.mark_automation_run_succeeded_at(
            lint_ok,
            OffsetDateTime::from_unix_timestamp(1_700_000_030).unwrap(),
        )
        .unwrap();

        let lint_fail_1 = repo
            .start_automation_run_at(
                "lint",
                OffsetDateTime::from_unix_timestamp(1_700_000_100).unwrap(),
            )
            .unwrap();
        repo.mark_automation_run_failed_at(
            lint_fail_1,
            OffsetDateTime::from_unix_timestamp(1_700_000_120).unwrap(),
            "lint timeout",
        )
        .unwrap();

        let lint_fail_2 = repo
            .start_automation_run_at(
                "lint",
                OffsetDateTime::from_unix_timestamp(1_700_000_200).unwrap(),
            )
            .unwrap();
        repo.mark_automation_run_failed_at(
            lint_fail_2,
            OffsetDateTime::from_unix_timestamp(1_700_000_220).unwrap(),
            "lint timeout again",
        )
        .unwrap();

        let maintenance_fail = repo
            .start_automation_run_at(
                "maintenance",
                OffsetDateTime::from_unix_timestamp(1_700_000_300).unwrap(),
            )
            .unwrap();
        repo.mark_automation_run_failed_at(
            maintenance_fail,
            OffsetDateTime::from_unix_timestamp(1_700_000_330).unwrap(),
            "db locked",
        )
        .unwrap();

        let failed = repo.list_recent_failed_automation_runs(3).unwrap();
        assert_eq!(failed.len(), 3);
        assert_eq!(failed[0].job_name, "maintenance");
        assert_eq!(failed[1].job_name, "lint");
        assert_eq!(failed[2].job_name, "lint");

        assert_eq!(
            repo.count_consecutive_automation_run_failures("lint")
                .unwrap(),
            2
        );
        assert_eq!(
            repo.count_consecutive_automation_run_failures("maintenance")
                .unwrap(),
            1
        );
        assert_eq!(
            repo.count_consecutive_automation_run_failures("batch-ingest")
                .unwrap(),
            0
        );

        let summaries = repo.list_automation_job_failure_summaries().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].job_name, "lint");
        assert_eq!(summaries[0].consecutive_failures, 2);
        assert_eq!(summaries[1].job_name, "maintenance");
        assert_eq!(summaries[1].consecutive_failures, 1);
    }

    #[test]
    fn storage_notion_cursor_roundtrip() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        // No cursor initially
        assert!(repo.get_notion_sync_cursor("x_bookmark").unwrap().is_none());

        let t1 = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        repo.upsert_notion_sync_cursor("x_bookmark", t1, 5).unwrap();

        let got = repo.get_notion_sync_cursor("x_bookmark").unwrap().unwrap();
        assert_eq!(got.unix_timestamp(), t1.unix_timestamp());

        // Upsert again with a newer timestamp; pages_synced accumulates
        let t2 = OffsetDateTime::from_unix_timestamp(1_700_001_000).unwrap();
        repo.upsert_notion_sync_cursor("x_bookmark", t2, 3).unwrap();

        let got2 = repo.get_notion_sync_cursor("x_bookmark").unwrap().unwrap();
        assert_eq!(got2.unix_timestamp(), t2.unix_timestamp());

        // Other db_id still missing
        assert!(repo.get_notion_sync_cursor("wechat").unwrap().is_none());
    }

    #[test]
    fn storage_notion_page_exists() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();

        let page_id = "abc-123";
        let source_id = wiki_core::SourceId(uuid::Uuid::new_v4());

        assert!(!repo.notion_page_exists(page_id).unwrap());

        repo.insert_notion_page_index(page_id, "x_bookmark", &source_id)
            .unwrap();
        assert!(repo.notion_page_exists(page_id).unwrap());

        // Inserting again (OR IGNORE) should not fail
        repo.insert_notion_page_index(page_id, "x_bookmark", &source_id)
            .unwrap();
        assert!(repo.notion_page_exists(page_id).unwrap());

        // Different page_id not found
        assert!(!repo.notion_page_exists("other-page").unwrap());
    }

    #[test]
    fn storage_notion_page_index_canonicalizes_hyphens_and_case() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let source_id = wiki_core::SourceId(uuid::Uuid::new_v4());

        repo.insert_notion_page_index("1A9701074B688103B989FBD0CFB8343A", "wechat", &source_id)
            .unwrap();

        assert!(repo
            .notion_page_exists("1a970107-4b68-8103-b989-fbd0cfb8343a")
            .unwrap());

        let count: i64 = rusqlite::Connection::open(&db)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM notion_page_index", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    fn canonical_alias(
        alias_text: &str,
        normalized_alias_key: &str,
        canonical_page_id: PageId,
        canonical_title: &str,
        scope: Scope,
        confidence: f64,
        at: OffsetDateTime,
    ) -> CanonicalAliasMapping {
        CanonicalAliasMapping {
            alias_text: alias_text.into(),
            normalized_alias_key: normalized_alias_key.into(),
            canonical_page_id,
            canonical_title: canonical_title.into(),
            entry_type: EntryType::Concept,
            scope,
            source: "unit-test".into(),
            confidence,
            created_at: at,
            updated_at: at,
        }
    }

    #[test]
    fn canonical_alias_upsert_is_idempotent_and_updates_existing_key() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let scope = Scope::Shared {
            team_id: "wiki".into(),
        };
        let page_id = PageId(uuid::Uuid::new_v4());
        let created_at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let updated_at = OffsetDateTime::from_unix_timestamp(1_700_000_120).unwrap();

        let first = canonical_alias(
            "MCP connectors",
            "mcpconnectors",
            page_id,
            "MCP 协议",
            scope.clone(),
            0.8,
            created_at,
        );
        repo.upsert_canonical_alias(&first).unwrap();

        let mut second = first.clone();
        second.alias_text = "MCP Connectors".into();
        second.canonical_title = "MCP 协议 canonical".into();
        second.confidence = 0.95;
        second.source = "resolver".into();
        second.updated_at = updated_at;
        repo.upsert_canonical_alias(&second).unwrap();

        let found = repo
            .find_canonical_alias(&scope, "mcpconnectors")
            .unwrap()
            .unwrap();
        assert_eq!(found.alias_text, "MCP Connectors");
        assert_eq!(found.canonical_page_id, page_id);
        assert_eq!(found.canonical_title, "MCP 协议 canonical");
        assert_eq!(found.confidence, 0.95);
        assert_eq!(found.source, "resolver");
        assert_eq!(found.created_at, created_at);
        assert_eq!(found.updated_at, updated_at);

        let count: i64 = rusqlite::Connection::open(&db)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM wiki_canonical_alias", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn canonical_alias_lookup_is_scoped_by_normalized_key() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let shared = Scope::Shared {
            team_id: "wiki".into(),
        };
        let private = Scope::Private {
            agent_id: "cli".into(),
        };
        let shared_page = PageId(uuid::Uuid::new_v4());
        let private_page = PageId(uuid::Uuid::new_v4());
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        repo.upsert_canonical_alias(&canonical_alias(
            "MCP 协议",
            "mcp",
            shared_page,
            "MCP 协议",
            shared.clone(),
            0.9,
            at,
        ))
        .unwrap();
        repo.upsert_canonical_alias(&canonical_alias(
            "MCP private",
            "mcp",
            private_page,
            "Private MCP",
            private.clone(),
            0.7,
            at,
        ))
        .unwrap();

        let shared_hit = repo.find_canonical_alias(&shared, "mcp").unwrap().unwrap();
        let private_hit = repo.find_canonical_alias(&private, "mcp").unwrap().unwrap();
        assert_eq!(shared_hit.canonical_page_id, shared_page);
        assert_eq!(private_hit.canonical_page_id, private_page);
        assert!(repo
            .find_canonical_alias(&shared, "missing")
            .unwrap()
            .is_none());
    }

    #[test]
    fn canonical_aliases_can_be_listed_by_page_ids() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let scope = Scope::Shared {
            team_id: "wiki".into(),
        };
        let page_a = PageId(uuid::Uuid::new_v4());
        let page_b = PageId(uuid::Uuid::new_v4());
        let page_c = PageId(uuid::Uuid::new_v4());
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        repo.upsert_canonical_alias(&canonical_alias(
            "MCP connectors",
            "mcpconnectors",
            page_a,
            "MCP 协议",
            scope.clone(),
            0.9,
            at,
        ))
        .unwrap();
        repo.upsert_canonical_alias(&canonical_alias(
            "MCP连接器",
            "mcp连接器",
            page_a,
            "MCP 协议",
            scope.clone(),
            0.9,
            at,
        ))
        .unwrap();
        repo.upsert_canonical_alias(&canonical_alias(
            "Claude Code",
            "claudecode",
            page_b,
            "Claude Code",
            scope,
            0.8,
            at,
        ))
        .unwrap();

        let aliases = repo
            .list_canonical_aliases_for_pages(&[page_a, page_c])
            .unwrap();
        let keys = aliases
            .iter()
            .map(|alias| alias.normalized_alias_key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(keys, vec!["mcpconnectors", "mcp连接器"]);
        assert!(aliases
            .iter()
            .all(|alias| alias.canonical_page_id == page_a));
    }

    #[test]
    fn canonical_aliases_can_be_listed_by_scope() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let shared = Scope::Shared {
            team_id: "wiki".into(),
        };
        let private = Scope::Private {
            agent_id: "cli".into(),
        };
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        repo.upsert_canonical_alias(&canonical_alias(
            "MCP connectors",
            "mcpconnectors",
            PageId(uuid::Uuid::new_v4()),
            "MCP 协议",
            shared.clone(),
            0.9,
            at,
        ))
        .unwrap();
        repo.upsert_canonical_alias(&canonical_alias(
            "MCP private",
            "mcpprivate",
            PageId(uuid::Uuid::new_v4()),
            "MCP private",
            private.clone(),
            0.9,
            at,
        ))
        .unwrap();

        let shared_aliases = repo.list_canonical_aliases_for_scope(&shared).unwrap();
        let private_aliases = repo.list_canonical_aliases_for_scope(&private).unwrap();

        assert_eq!(shared_aliases.len(), 1);
        assert_eq!(shared_aliases[0].alias_text, "MCP connectors");
        assert_eq!(private_aliases.len(), 1);
        assert_eq!(private_aliases[0].alias_text, "MCP private");
    }

    #[test]
    fn snapshot_outbox_and_aliases_commit_together() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("wiki.db");
        let repo = SqliteRepository::open(&db).unwrap();
        let scope = Scope::Shared {
            team_id: "wiki".into(),
        };
        let page_id = PageId(uuid::Uuid::new_v4());
        let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mut snapshot = StorageSnapshot::default();
        snapshot
            .pages
            .push(WikiPage::new("MCP 协议", "", scope.clone()));
        let event = WikiEvent::PageWritten { page_id, at };
        let alias = canonical_alias(
            "MCP connectors",
            "mcpconnectors",
            page_id,
            "MCP 协议",
            scope.clone(),
            0.9,
            at,
        );

        repo.save_snapshot_and_append_outbox_with_aliases(&snapshot, &[event], &[alias])
            .unwrap();

        assert_eq!(repo.load_snapshot().unwrap().pages.len(), 1);
        assert!(repo
            .export_outbox_ndjson()
            .unwrap()
            .contains("page_written"));
        assert!(repo
            .find_canonical_alias(&scope, "mcpconnectors")
            .unwrap()
            .is_some());
    }
}
