use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use time::{format_description, format_description::well_known::Rfc3339, OffsetDateTime, PrimitiveDateTime};
use wiki_core::{AuditRecord, Claim, Entity, RawArtifact, TypedEdge, WikiEvent, WikiPage};

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

pub trait WikiRepository {
    fn load_snapshot(&self) -> Result<StorageSnapshot, StorageError>;
    fn save_snapshot(&self, snapshot: &StorageSnapshot) -> Result<(), StorageError>;
    fn append_outbox(&self, event: &WikiEvent) -> Result<(), StorageError>;
    fn export_outbox_ndjson(&self) -> Result<String, StorageError>;
    fn export_outbox_ndjson_from_id(&self, last_id: i64) -> Result<String, StorageError>;
    fn mark_outbox_processed(
        &self,
        up_to_id: i64,
        consumer_tag: &str,
    ) -> Result<usize, StorageError>;
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

    pub fn get_outbox_consumer_progress(
        &self,
        consumer_tag: &str,
    ) -> Result<OutboxConsumerProgress, StorageError> {
        let stats = self.get_outbox_stats()?;
        let row = self.conn.query_row(
            "SELECT id, processed_at
             FROM wiki_outbox
             WHERE consumer_tag = ?1 AND processed_at IS NOT NULL
             ORDER BY id DESC
             LIMIT 1",
            params![consumer_tag],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        );
        let (acked_up_to_id, acked_at) = match row {
            Ok((id, processed_at_raw)) => (Some(id), Some(parse_time(&processed_at_raw)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => (None, None),
            Err(e) => return Err(StorageError::Db(e)),
        };
        let backlog_events = match acked_up_to_id {
            Some(id) => (stats.head_id - id).max(0),
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
                continue;
            };
            if v.len() != query.len() {
                continue;
            }
            let vn = l2_norm(&v);
            if vn <= 1e-12 {
                continue;
            }
            let dot: f32 = query.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
            let c = dot / (qn * vn);
            scored.push((doc_id, c));
        }
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        scored.truncate(limit);
        Ok(scored)
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
        let duration_ms = i64::try_from((finished_at - started_at).whole_milliseconds())
            .map_err(|_| {
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
            Some(status) => self.conn.query_row(sql, params![job_name, status.as_str()], |row| {
                decode_automation_run_row(row)
            }),
            None => self.conn.query_row(sql, params![job_name], |row| decode_automation_run_row(row)),
        };
        match result {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Db(e)),
        }
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
        .map_err(|err| StorageError::InvalidAutomationRunState(format!("invalid timestamp {value:?}: {err}")))
}

fn decode_automation_run_row(row: &rusqlite::Row<'_>) -> Result<AutomationRunRecord, rusqlite::Error> {
    let id: i64 = row.get(0)?;
    let job_name: String = row.get(1)?;
    let started_at_raw: String = row.get(2)?;
    let finished_at_raw: Option<String> = row.get(3)?;
    let status_raw: String = row.get(4)?;
    let duration_ms: Option<i64> = row.get(5)?;
    let error_summary: Option<String> = row.get(6)?;
    let heartbeat_at_raw: String = row.get(7)?;

    let started_at = parse_time(&started_at_raw)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(err)))?;
    let finished_at = match finished_at_raw {
        Some(value) => Some(
            parse_time(&value).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err))
            })?,
        ),
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
        let payload = serde_json::to_string(snapshot)?;
        self.conn.execute(
            "INSERT INTO wiki_state(id, payload_json) VALUES(1, ?1)
             ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json",
            params![payload],
        )?;
        Ok(())
    }

    fn append_outbox(&self, event: &WikiEvent) -> Result<(), StorageError> {
        let payload = serde_json::to_string(event)?;
        self.conn.execute(
            "INSERT INTO wiki_outbox(event_json) VALUES(?1)",
            params![payload],
        )?;
        Ok(())
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

    fn mark_outbox_processed(
        &self,
        up_to_id: i64,
        consumer_tag: &str,
    ) -> Result<usize, StorageError> {
        let n = self.conn.execute(
            "UPDATE wiki_outbox
             SET processed_at = datetime('now'), consumer_tag = ?2
             WHERE id <= ?1 AND processed_at IS NULL",
            params![up_to_id, consumer_tag],
        )?;
        Ok(n)
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

        let run_id = repo
            .start_automation_run_at("batch-sync", start)
            .unwrap();
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

        let ok_id = repo.start_automation_run_at("batch-sync", start_ok).unwrap();
        repo.mark_automation_run_succeeded_at(ok_id, finish_ok)
            .unwrap();

        let fail_id = repo.start_automation_run_at("batch-sync", start_fail).unwrap();
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
}
