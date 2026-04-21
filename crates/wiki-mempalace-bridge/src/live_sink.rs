use crate::{MempalaceError, MempalaceWikiSink};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use rust_mempalace::{db, service};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use wiki_core::{Claim, ClaimId, Scope, SourceId};

pub struct LiveMempalaceSink {
    conn: Mutex<Connection>,
    bank_id: String,
}

impl LiveMempalaceSink {
    pub fn open(palace_db_path: &std::path::Path, bank_id: &str) -> Result<Self, MempalaceError> {
        let conn = db::open(palace_db_path)
            .map_err(|e| MempalaceError::Backend(format!("open palace db: {e}")))?;
        db::init_schema(&conn)
            .map_err(|e| MempalaceError::Backend(format!("init palace schema: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
            bank_id: bank_id.to_string(),
        })
    }

    fn with_conn<F, R>(&self, f: F) -> Result<R, MempalaceError>
    where
        F: FnOnce(&Connection) -> Result<R, MempalaceError>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|e| MempalaceError::Backend(format!("lock: {e}")))?;
        f(&conn)
    }

    fn insert_drawer(
        &self,
        wing: &str,
        hall: &str,
        room: &str,
        source_path: &str,
        content: &str,
    ) -> Result<(), MempalaceError> {
        self.with_conn(|conn| {
            let content_hash = sha256_hex(source_path, content);
            let exists = conn
                .query_row(
                    "SELECT 1 FROM drawers WHERE content_hash = ?1 LIMIT 1",
                    params![content_hash],
                    |_| Ok(1i64),
                )
                .optional()
                .map_err(|e| MempalaceError::Backend(e.to_string()))?
                .is_some();
            if exists {
                return Ok(());
            }
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO drawers(wing, hall, room, source_path, content, content_hash, bank_id, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![wing, hall, room, source_path, content, content_hash, self.bank_id, now],
            )
            .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            let row_id = conn.last_insert_rowid();
            service::upsert_vector(conn, row_id, content)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(())
        })
    }
}

impl MempalaceWikiSink for LiveMempalaceSink {
    fn on_claim_upserted(&self, claim: &Claim) -> Result<(), MempalaceError> {
        let room = short_id(&claim.id);
        let source_path = format!("wiki://claim/{}", claim.id.0);
        self.insert_drawer(
            "wiki_claims",
            "hall_facts",
            &room,
            &source_path,
            &claim.text,
        )
    }

    fn on_claim_event(&self, _claim_id: ClaimId) -> Result<(), MempalaceError> {
        Ok(())
    }

    fn on_claim_superseded(&self, old: ClaimId, new: ClaimId) -> Result<(), MempalaceError> {
        self.with_conn(|conn| {
            let subject = format!("claim:{}", new.0);
            let object = format!("claim:{}", old.0);
            service::kg_add(conn, &subject, "supersedes", &object, None, None)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            service::kg_invalidate(conn, &format!("claim:{}", old.0), "is_active", "true", None)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(())
        })
    }

    fn on_source_linked(
        &self,
        source_id: SourceId,
        claim_id: ClaimId,
    ) -> Result<(), MempalaceError> {
        self.with_conn(|conn| {
            let subject = format!("source:{}", source_id.0);
            let object = format!("claim:{}", claim_id.0);
            service::kg_add(conn, &subject, "supports", &object, None, None)
                .map_err(|e| MempalaceError::Backend(e.to_string()))
        })
    }

    fn on_source_ingested(&self, source_id: SourceId) -> Result<(), MempalaceError> {
        let source_path = format!("wiki://source/{}", source_id.0);
        let room = short_id_raw(source_id.0);
        self.insert_drawer(
            "wiki_sources",
            "hall_events",
            &room,
            &source_path,
            &format!("Source ingested: {}", source_id.0),
        )
    }

    fn scope_filter(&self, scope: &Scope) -> bool {
        match scope {
            Scope::Private { agent_id } => agent_id == &self.bank_id,
            Scope::Shared { team_id } => team_id == &self.bank_id,
        }
    }
}

fn sha256_hex(path: &str, content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn short_id(cid: &ClaimId) -> String {
    short_id_raw(cid.0)
}

fn short_id_raw(uuid: uuid::Uuid) -> String {
    uuid.to_string()[..8].to_string()
}
