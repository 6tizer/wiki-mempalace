//! Mempalace MCP 工具的抽象层：把 wiki-cli 的 10 个 `mempalace_*` 工具
//! 全部收归到 `MempalaceTools` trait，避免 wiki-cli 直接依赖 rust-mempalace。
//!
//! - `NoopMempalaceTools`：无操作实现（默认 feature / 单测）。
//! - `LiveMempalaceTools`：真实实现（`feature = "live"`），见 `live_tools.rs`。

use serde_json::{json, Value};

use crate::MempalaceError;

/// 10 个 mempalace_* MCP 工具的统一抽象。
///
/// 所有方法返回 `Result<Value, MempalaceError>`，JSON 结构与重构前完全一致。
pub trait MempalaceTools: Send + Sync {
    fn status(&self) -> Result<Value, MempalaceError>;
    #[allow(clippy::too_many_arguments)]
    fn search(
        &self,
        query: &str,
        wing: Option<&str>,
        hall: Option<&str>,
        room: Option<&str>,
        bank_id: Option<&str>,
        limit: usize,
        explain: bool,
    ) -> Result<Value, MempalaceError>;
    fn wake_up(&self, wing: Option<&str>, bank_id: Option<&str>) -> Result<Value, MempalaceError>;
    fn taxonomy(&self, bank_id: Option<&str>) -> Result<Value, MempalaceError>;
    fn traverse(
        &self,
        wing: &str,
        room: &str,
        bank_id: Option<&str>,
    ) -> Result<Value, MempalaceError>;
    fn kg_query(&self, subject: &str, as_of: Option<&str>) -> Result<Value, MempalaceError>;
    fn kg_timeline(&self, subject: &str) -> Result<Value, MempalaceError>;
    fn kg_stats(&self) -> Result<Value, MempalaceError>;
    fn reflect(
        &self,
        query: &str,
        search_limit: usize,
        bank_id: Option<&str>,
    ) -> Result<Value, MempalaceError>;
    fn extract(&self, text: Option<&str>, drawer_id: Option<i64>) -> Result<Value, MempalaceError>;
}

/// 无操作实现：所有字段为空 / 零值，保证不触碰 rust-mempalace。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopMempalaceTools;

impl MempalaceTools for NoopMempalaceTools {
    fn status(&self) -> Result<Value, MempalaceError> {
        Ok(json!({"drawers": 0, "wings": 0, "tunnels": 0, "kg_facts": 0}))
    }

    fn search(
        &self,
        _query: &str,
        _wing: Option<&str>,
        _hall: Option<&str>,
        _room: Option<&str>,
        _bank_id: Option<&str>,
        _limit: usize,
        _explain: bool,
    ) -> Result<Value, MempalaceError> {
        Ok(json!({"results": []}))
    }

    fn wake_up(
        &self,
        _wing: Option<&str>,
        _bank_id: Option<&str>,
    ) -> Result<Value, MempalaceError> {
        Ok(json!({"text": ""}))
    }

    fn taxonomy(&self, _bank_id: Option<&str>) -> Result<Value, MempalaceError> {
        Ok(json!({"taxonomy": []}))
    }

    fn traverse(
        &self,
        _wing: &str,
        _room: &str,
        _bank_id: Option<&str>,
    ) -> Result<Value, MempalaceError> {
        Ok(json!({"links": []}))
    }

    fn kg_query(&self, _subject: &str, _as_of: Option<&str>) -> Result<Value, MempalaceError> {
        Ok(json!({"facts": []}))
    }

    fn kg_timeline(&self, _subject: &str) -> Result<Value, MempalaceError> {
        Ok(json!({"timeline": []}))
    }

    fn kg_stats(&self) -> Result<Value, MempalaceError> {
        Ok(json!({"facts": 0, "subjects": 0, "predicates": 0, "active_facts": 0}))
    }

    fn reflect(
        &self,
        _query: &str,
        _search_limit: usize,
        _bank_id: Option<&str>,
    ) -> Result<Value, MempalaceError> {
        Ok(json!({"text": ""}))
    }

    fn extract(
        &self,
        _text: Option<&str>,
        _drawer_id: Option<i64>,
    ) -> Result<Value, MempalaceError> {
        Ok(json!({"kg_facts_added": 0}))
    }
}

/// 工厂函数：根据是否启用 `live` feature 返回对应实现。
///
/// - `live` 开启 → `LiveMempalaceTools`（连接真实 palace.db）
/// - 默认 → `NoopMempalaceTools`
#[cfg(not(feature = "live"))]
pub fn make_tools(_palace_root: Option<&str>) -> Result<Box<dyn MempalaceTools>, MempalaceError> {
    Ok(Box::new(NoopMempalaceTools))
}

#[cfg(feature = "live")]
pub fn make_tools(palace_root: Option<&str>) -> Result<Box<dyn MempalaceTools>, MempalaceError> {
    use crate::LiveMempalaceTools;
    let tools = LiveMempalaceTools::new(palace_root)?;
    Ok(Box::new(tools))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助：断言 Value 是对象且包含指定 key
    fn assert_has_keys(val: &Value, keys: &[&str]) {
        let obj = val.as_object().expect("should be a JSON object");
        for k in keys {
            assert!(obj.contains_key(*k), "missing key: {k}");
        }
    }

    #[test]
    fn noop_status_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.status().unwrap();
        assert_has_keys(&v, &["drawers", "wings", "tunnels", "kg_facts"]);
        assert_eq!(v["drawers"], 0);
    }

    #[test]
    fn noop_search_shape() {
        let tools = NoopMempalaceTools;
        let v = tools
            .search("test", None, None, None, None, 5, false)
            .unwrap();
        assert_has_keys(&v, &["results"]);
        assert!(v["results"].as_array().unwrap().is_empty());
    }

    #[test]
    fn noop_wake_up_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.wake_up(None, None).unwrap();
        assert_has_keys(&v, &["text"]);
    }

    #[test]
    fn noop_taxonomy_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.taxonomy(None).unwrap();
        assert_has_keys(&v, &["taxonomy"]);
    }

    #[test]
    fn noop_traverse_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.traverse("w", "r", None).unwrap();
        assert_has_keys(&v, &["links"]);
    }

    #[test]
    fn noop_kg_query_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.kg_query("s", None).unwrap();
        assert_has_keys(&v, &["facts"]);
    }

    #[test]
    fn noop_kg_timeline_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.kg_timeline("s").unwrap();
        assert_has_keys(&v, &["timeline"]);
    }

    #[test]
    fn noop_kg_stats_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.kg_stats().unwrap();
        assert_has_keys(&v, &["facts", "subjects", "predicates", "active_facts"]);
        assert_eq!(v["facts"], 0);
    }

    #[test]
    fn noop_reflect_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.reflect("q", 8, None).unwrap();
        assert_has_keys(&v, &["text"]);
    }

    #[test]
    fn noop_extract_shape() {
        let tools = NoopMempalaceTools;
        let v = tools.extract(Some("text"), None).unwrap();
        assert_has_keys(&v, &["kg_facts_added"]);
        assert_eq!(v["kg_facts_added"], 0);
    }
}
