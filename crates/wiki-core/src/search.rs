//! 混合检索：三路有序 doc id 列表 → Reciprocal Rank Fusion（RRF）。

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::search_ports::SearchPorts;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedDoc {
    pub id: String,
    pub rrf_score: f64,
}

/// 标准 RRF：`score(d) = sum_i 1/(k + rank_i(d))`，仅出现在某些流中的 rank 可视为缺失（此处省略该项）。
pub fn reciprocal_rank_fusion(rank_lists: &[Vec<String>], k: f64) -> Vec<RankedDoc> {
    let mut acc: HashMap<String, f64> = HashMap::new();
    for ranks in rank_lists {
        for (idx, id) in ranks.iter().enumerate() {
            let rank = (idx + 1) as f64;
            *acc.entry(id.clone()).or_insert(0.0) += 1.0 / (k + rank);
        }
    }
    let mut v: Vec<RankedDoc> = acc
        .into_iter()
        .map(|(id, rrf_score)| RankedDoc { id, rrf_score })
        .collect();
    v.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    v
}

/// 多源召回的组合端口。将 wiki 内部召回和 mempalace 外部召回统一融合。
///
/// 融合策略：各路分别召回，然后按 doc id 去重（保留首次出现），
/// 最后交给 RRF 层做全局排序。
pub struct CompositeSearchPorts<'a> {
    ports: Vec<Box<dyn SearchPorts + 'a>>,
    config: FusionConfig,
}

impl<'a> CompositeSearchPorts<'a> {
    pub fn new(ports: Vec<Box<dyn SearchPorts + 'a>>, config: FusionConfig) -> Self {
        Self { ports, config }
    }

    /// 仅使用 wiki 内部端口（向后兼容）
    pub fn wiki_only(inner: Box<dyn SearchPorts + 'a>) -> Self {
        Self::new(vec![inner], FusionConfig::default())
    }
}

impl<'a> SearchPorts for CompositeSearchPorts<'a> {
    fn bm25_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        let per_source = self.config.per_source_limit;
        for port in &self.ports {
            for id in port.bm25_ranked_ids(query, per_source) {
                if seen.insert(id.clone()) {
                    result.push(id);
                    if result.len() >= limit {
                        return result;
                    }
                }
            }
        }
        result
    }

    fn vector_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        let per_source = self.config.per_source_limit;
        for port in &self.ports {
            for id in port.vector_ranked_ids(query, per_source) {
                if seen.insert(id.clone()) {
                    result.push(id);
                    if result.len() >= limit {
                        return result;
                    }
                }
            }
        }
        result
    }

    fn graph_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        let per_source = self.config.per_source_limit;
        for port in &self.ports {
            for id in port.graph_ranked_ids(query, per_source) {
                if seen.insert(id.clone()) {
                    result.push(id);
                    if result.len() >= limit {
                        return result;
                    }
                }
            }
        }
        result
    }
}

/// 融合配置：控制多源召回的行为。
#[derive(Debug, Clone)]
pub struct FusionConfig {
    /// 是否启用外部源（mempalace）
    pub external_enabled: bool,
    /// 各路召回的 per-source limit
    pub per_source_limit: usize,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            external_enabled: false,
            per_source_limit: 32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_orders_by_fusion() {
        let bm25 = vec!["a".into(), "b".into(), "c".into()];
        let vector = vec!["b".into(), "a".into()];
        let graph = vec!["c".into(), "a".into()];
        let out = reciprocal_rank_fusion(&[bm25, vector, graph], 60.0);
        assert!(!out.is_empty());
        // "a" appears in all three → should be strong
        assert_eq!(out[0].id, "a");
    }

    #[test]
    fn composite_search_deduplicates_across_sources() {
        struct Mock(Vec<String>);
        impl SearchPorts for Mock {
            fn bm25_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
            fn vector_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
            fn graph_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
        }

        let a = Mock(vec!["x".into(), "y".into()]);
        let b = Mock(vec!["y".into(), "z".into()]);
        let composite =
            CompositeSearchPorts::new(vec![Box::new(a), Box::new(b)], FusionConfig::default());
        let ids = composite.bm25_ranked_ids("q", 10);
        assert_eq!(ids, vec!["x", "y", "z"]);
    }

    #[test]
    fn composite_search_respects_limit() {
        struct Mock(Vec<String>);
        impl SearchPorts for Mock {
            fn bm25_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
            fn vector_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
            fn graph_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
        }

        let a = Mock(vec!["a1".into(), "a2".into(), "a3".into()]);
        let b = Mock(vec!["b1".into(), "b2".into()]);
        let composite =
            CompositeSearchPorts::new(vec![Box::new(a), Box::new(b)], FusionConfig::default());
        let ids = composite.vector_ranked_ids("q", 4);
        assert_eq!(ids.len(), 4);
        assert_eq!(ids, vec!["a1", "a2", "a3", "b1"]);
    }

    #[test]
    fn composite_search_preserves_order() {
        struct Mock(Vec<String>);
        impl SearchPorts for Mock {
            fn bm25_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
            fn vector_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
            fn graph_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
                self.0.clone()
            }
        }

        let a = Mock(vec!["first".into(), "second".into()]);
        let b = Mock(vec!["third".into()]);
        let composite =
            CompositeSearchPorts::new(vec![Box::new(a), Box::new(b)], FusionConfig::default());
        let ids = composite.graph_ranked_ids("q", 10);
        assert_eq!(ids, vec!["first", "second", "third"]);
    }

    #[test]
    fn fusion_config_default_has_no_external() {
        let cfg = FusionConfig::default();
        assert!(!cfg.external_enabled);
        assert_eq!(cfg.per_source_limit, 32);
    }

    /// 空 ports 列表时，各路召回均返回空。
    #[test]
    fn composite_search_empty_ports_returns_empty() {
        let composite = CompositeSearchPorts::new(vec![], FusionConfig::default());
        assert!(composite.bm25_ranked_ids("q", 10).is_empty());
        assert!(composite.vector_ranked_ids("q", 10).is_empty());
        assert!(composite.graph_ranked_ids("q", 10).is_empty());
    }
}
