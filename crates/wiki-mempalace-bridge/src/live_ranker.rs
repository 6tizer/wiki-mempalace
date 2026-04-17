use crate::MempalaceGraphRanker;
use rusqlite::Connection;
use rust_mempalace::service::{self, RetrievalConfig};
use std::sync::Mutex;

pub struct LiveMempalaceGraphRanker {
    conn: Mutex<Connection>,
    retrieval: RetrievalConfig,
    bank_id: Option<String>,
}

impl LiveMempalaceGraphRanker {
    pub fn open(
        palace_db_path: &std::path::Path,
        bank_id: Option<String>,
    ) -> Result<Self, crate::MempalaceError> {
        let conn = rust_mempalace::db::open(palace_db_path)
            .map_err(|e| crate::MempalaceError::Backend(format!("open palace db: {e}")))?;
        rust_mempalace::db::init_schema(&conn)
            .map_err(|e| crate::MempalaceError::Backend(format!("init schema: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
            retrieval: RetrievalConfig {
                lexical_weight: 1.0,
                vector_weight: 1.3,
                rrf_k: 60.0,
                rrf_weight: 18.0,
            },
            bank_id,
        })
    }

    pub fn with_retrieval(mut self, cfg: RetrievalConfig) -> Self {
        self.retrieval = cfg;
        self
    }
}

impl MempalaceGraphRanker for LiveMempalaceGraphRanker {
    fn graph_rank_extras(&self, query: &str, limit: usize) -> Vec<String> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let rows = service::search_with_options(
            &conn,
            query,
            None,
            None,
            None,
            self.bank_id.as_deref(),
            limit,
            &self.retrieval,
            false,
        );
        let rows = match rows {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut doc_ids: Vec<String> = Vec::new();
        for row in &rows {
            doc_ids.push(format!("mp_drawer:{}", row.id));
        }

        let kg_facts = service::kg_query(&conn, query, None).unwrap_or_default();
        for fact in &kg_facts {
            let id = format!("mp_kg:{}:{}", fact.subject, fact.predicate);
            if !doc_ids.contains(&id) {
                doc_ids.push(id);
            }
        }

        doc_ids.truncate(limit);
        doc_ids
    }
}
