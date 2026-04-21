//! `LiveMempalaceTools`：通过 `rust-mempalace` 真实连接 palace.db 的实现。
//!
//! JSON 输出结构与重构前 wiki-cli mcp.rs 中的 `call_mempalace_tool` 完全一致，
//! 保证 MCP API 向后兼容。

use rusqlite::Connection;
use rust_mempalace::service::{self, AppConfig};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::{MempalaceError, MempalaceTools};

/// 持有 palace 连接和配置的真实实现。
pub struct LiveMempalaceTools {
    conn: Mutex<Connection>,
    config: AppConfig,
}

impl LiveMempalaceTools {
    /// 构造：打开 / 初始化 palace 数据库，加载配置。
    pub fn new(palace_root: Option<&str>) -> Result<Self, MempalaceError> {
        let root = palace_root.unwrap_or("~/.mempalace-rs");
        let expanded = if root.starts_with('~') {
            shellexpand::tilde(root).to_string()
        } else {
            root.to_string()
        };
        let root_path = PathBuf::from(&expanded);

        let palace = service::Palace::new(&expanded)
            .map_err(|e| MempalaceError::Backend(format!("Palace::new: {e}")))?;
        palace
            .init(None)
            .map_err(|e| MempalaceError::Backend(format!("palace init: {e}")))?;
        let conn = palace
            .open()
            .map_err(|e| MempalaceError::Backend(format!("palace open: {e}")))?;
        let config_path = root_path.join("config.toml");
        let config = service::load_config(&config_path);

        Ok(Self {
            conn: Mutex::new(conn),
            config,
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
}

impl MempalaceTools for LiveMempalaceTools {
    fn status(&self) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let s = service::status(conn).map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({
                "drawers": s.drawers,
                "wings": s.wings,
                "tunnels": s.tunnels,
                "kg_facts": s.kg_facts,
            }))
        })
    }

    fn search(
        &self,
        query: &str,
        wing: Option<&str>,
        hall: Option<&str>,
        room: Option<&str>,
        bank_id: Option<&str>,
        limit: usize,
        explain: bool,
    ) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let rows = service::search_with_options(
                conn,
                query,
                wing,
                hall,
                room,
                bank_id,
                limit,
                &self.config.retrieval,
                explain,
            )
            .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({
                "results": rows.iter().map(|r| json!({
                    "id": r.id,
                    "wing": r.wing,
                    "hall": r.hall,
                    "room": r.room,
                    "bank_id": r.bank_id,
                    "source_path": r.source_path,
                    "snippet": r.snippet,
                    "score": r.score,
                    "explain": r.explain,
                })).collect::<Vec<_>>()
            }))
        })
    }

    fn wake_up(&self, wing: Option<&str>, bank_id: Option<&str>) -> Result<Value, MempalaceError> {
        // identity.json 与 palace.db 同目录
        let identity_path = {
            let conn = self
                .conn
                .lock()
                .map_err(|e| MempalaceError::Backend(format!("lock: {e}")))?;
            conn.path()
                .map(|p| {
                    let db_path = std::path::Path::new(p);
                    db_path.parent().unwrap_or(db_path).join("identity.json")
                })
                .unwrap_or_else(|| std::path::PathBuf::from("identity.json"))
        };
        self.with_conn(|conn| {
            let text = service::wake_up(conn, &identity_path, wing, bank_id)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({"text": text}))
        })
    }

    fn taxonomy(&self, bank_id: Option<&str>) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let rows = service::taxonomy(conn, bank_id)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({
                "taxonomy": rows.iter().map(|r| json!({
                    "wing": r.wing,
                    "hall": r.hall,
                    "room": r.room,
                    "count": r.count,
                })).collect::<Vec<_>>()
            }))
        })
    }

    fn traverse(
        &self,
        wing: &str,
        room: &str,
        bank_id: Option<&str>,
    ) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let rows = service::traverse(conn, wing, room, bank_id)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({
                "links": rows.iter().map(|r| json!({
                    "kind": r.kind,
                    "from_wing": r.from_wing,
                    "from_room": r.from_room,
                    "to_wing": r.to_wing,
                    "to_room": r.to_room,
                })).collect::<Vec<_>>()
            }))
        })
    }

    fn kg_query(&self, subject: &str, as_of: Option<&str>) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let rows = service::kg_query(conn, subject, as_of)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({
                "facts": rows.iter().map(|r| json!({
                    "id": r.id,
                    "subject": r.subject,
                    "predicate": r.predicate,
                    "object": r.object,
                    "valid_from": r.valid_from,
                    "valid_to": r.valid_to,
                    "source_drawer_id": r.source_drawer_id,
                })).collect::<Vec<_>>()
            }))
        })
    }

    fn kg_timeline(&self, subject: &str) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let rows = service::kg_timeline(conn, subject)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({
                "timeline": rows.iter().map(|r| json!({
                    "id": r.id,
                    "subject": r.subject,
                    "predicate": r.predicate,
                    "object": r.object,
                    "valid_from": r.valid_from,
                    "valid_to": r.valid_to,
                    "source_drawer_id": r.source_drawer_id,
                })).collect::<Vec<_>>()
            }))
        })
    }

    fn kg_stats(&self) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let s = service::kg_stats(conn).map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({
                "facts": s.facts,
                "subjects": s.subjects,
                "predicates": s.predicates,
                "active_facts": s.active_facts,
            }))
        })
    }

    fn reflect(
        &self,
        query: &str,
        search_limit: usize,
        bank_id: Option<&str>,
    ) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let text = service::reflect_answer(
                conn,
                &self.config.llm,
                &self.config.retrieval,
                query,
                bank_id,
                search_limit,
            )
            .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({"text": text}))
        })
    }

    fn extract(&self, text: Option<&str>, drawer_id: Option<i64>) -> Result<Value, MempalaceError> {
        self.with_conn(|conn| {
            let body = match (text, drawer_id) {
                (Some(t), None) => t.to_string(),
                (None, Some(id)) => service::drawer_content(conn, id)
                    .map_err(|e| MempalaceError::Backend(e.to_string()))?
                    .ok_or_else(|| MempalaceError::Backend("drawer id not found".into()))?,
                (None, None) => {
                    return Err(MempalaceError::Backend("provide text or drawer_id".into()))
                }
                (Some(_), Some(_)) => {
                    return Err(MempalaceError::Backend(
                        "use only one of text or drawer_id".into(),
                    ))
                }
            };
            let n = service::extract_to_kg(conn, &self.config.llm, &body)
                .map_err(|e| MempalaceError::Backend(e.to_string()))?;
            Ok(json!({"kg_facts_added": n}))
        })
    }
}
