use rusqlite::{params, Connection};
use rust_mempalace::service::{self, RetrievalConfig};
use std::sync::Mutex;
use wiki_core::SearchPorts;

pub struct MempalaceSearchPorts {
    conn: Mutex<Connection>,
    retrieval: RetrievalConfig,
    bank_id: Option<String>,
}

impl MempalaceSearchPorts {
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

impl SearchPorts for MempalaceSearchPorts {
    fn bm25_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
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
        match rows {
            Ok(r) => r
                .iter()
                .map(|row| format!("mp_drawer:{}", row.id))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn vector_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let q_vec = service::sparse_embedding(query);
        if q_vec.is_empty() {
            return Vec::new();
        }

        let mut stmt = match conn.prepare("SELECT drawer_id, vector_json FROM drawer_vectors") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let mut rows = match stmt.query(params![]) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut scored: Vec<(String, f64)> = Vec::new();
        while let Ok(Some(row)) = rows.next() {
            let drawer_id: i64 = match row.get(0) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let json_str: String = match row.get(1) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let doc_vec: std::collections::BTreeMap<String, f64> =
                match serde_json::from_str(&json_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
            let sim = cosine_sim_btree(&q_vec, &doc_vec);
            if sim > 0.0 {
                scored.push((format!("mp_drawer:{}", drawer_id), sim));
            }
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored.into_iter().map(|(id, _)| id).collect()
    }

    fn graph_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let facts = service::kg_query(&conn, query, None).unwrap_or_default();
        facts
            .iter()
            .take(limit)
            .map(|f| format!("mp_kg:{}:{}", f.subject, f.predicate))
            .collect()
    }
}

fn cosine_sim_btree(
    a: &std::collections::BTreeMap<String, f64>,
    b: &std::collections::BTreeMap<String, f64>,
) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    let (small, large) = if a.len() < b.len() { (a, b) } else { (b, a) };
    for (k, v) in small {
        if let Some(v2) = large.get(k) {
            s += v * v2;
        }
    }
    s
}
