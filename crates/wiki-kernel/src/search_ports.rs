//! 三路检索端口：BM25 / 向量 / 图 由外部实现；内存 stub 供开发与单测。

use std::collections::HashSet;

use crate::memory::InMemoryStore;
use wiki_core::{document_visible_to_viewer, ClaimId, EntityId, PageId, Scope};

pub use wiki_core::SearchPorts;

/// 空三路（用于只测 RRF  plumbing）。
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptySearchPorts;

impl SearchPorts for EmptySearchPorts {
    fn bm25_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
        Vec::new()
    }

    fn vector_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
        Vec::new()
    }

    fn graph_ranked_ids(&self, _query: &str, _limit: usize) -> Vec<String> {
        Vec::new()
    }
}

fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 1)
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn score_text_lc(haystack_lc: &str, tokens: &[String]) -> usize {
    tokens
        .iter()
        .filter(|t| haystack_lc.contains(t.as_str()))
        .count()
}

/// 基于子串/token 重叠的内存 stub：BM25 与 vector 两路顺序刻意不同以检验 RRF。
#[derive(Debug, Clone)]
pub struct InMemorySearchPorts<'a> {
    pub store: &'a InMemoryStore,
    /// 若 `Some`，仅索引该视角可见的文档（与 `QueryContext::viewer_scope` 对齐）。
    pub viewer_scope: Option<Scope>,
}

impl<'a> InMemorySearchPorts<'a> {
    pub fn new(store: &'a InMemoryStore, viewer_scope: Option<Scope>) -> Self {
        Self {
            store,
            viewer_scope,
        }
    }

    fn scope_ok(&self, doc: &Scope) -> bool {
        match &self.viewer_scope {
            None => true,
            Some(v) => document_visible_to_viewer(doc, v),
        }
    }

    fn collect_doc_scores(&self, tokens: &[String]) -> Vec<(String, usize)> {
        let mut scored: Vec<(String, usize)> = Vec::new();
        for c in self.store.claims.values() {
            if c.stale || !self.scope_ok(&c.scope) {
                continue;
            }
            let t = c.text.to_ascii_lowercase();
            let s = score_text_lc(&t, tokens);
            if s > 0 {
                scored.push((format_claim_doc_id(c.id), s));
            }
        }
        for p in self.store.pages.values() {
            if !self.scope_ok(&p.scope) {
                continue;
            }
            let blob = format!("{} {}", p.title, p.markdown).to_ascii_lowercase();
            let s = score_text_lc(&blob, tokens);
            if s > 0 {
                scored.push((format_page_doc_id(p.id), s));
            }
        }
        scored
    }
}

impl SearchPorts for InMemorySearchPorts<'_> {
    fn bm25_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let tokens = query_tokens(query);
        let mut scored: Vec<(String, usize)> = self.collect_doc_scores(&tokens);
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.into_iter().map(|(id, _)| id).take(limit).collect()
    }

    fn vector_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let tokens = query_tokens(query);
        let mut scored: Vec<(String, usize)> = self.collect_doc_scores(&tokens);
        // 同分按 id 逆序，模拟与 BM25 不同的重排
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
        scored.into_iter().map(|(id, _)| id).take(limit).collect()
    }

    fn graph_ranked_ids(&self, query: &str, limit: usize) -> Vec<String> {
        let tokens = query_tokens(query);
        let mut scored: Vec<(String, usize)> = Vec::new();
        for e in self.store.entities.values() {
            if !self.scope_ok(&e.scope) {
                continue;
            }
            let label_lc = e.label.to_ascii_lowercase();
            let s = score_text_lc(&label_lc, &tokens);
            if s > 0 {
                scored.push((format_entity_doc_id(e.id), s));
            }
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.into_iter().map(|(id, _)| id).take(limit).collect()
    }
}

pub fn format_claim_doc_id(id: ClaimId) -> String {
    format!("claim:{}", id.0)
}

pub fn format_page_doc_id(id: PageId) -> String {
    format!("page:{}", id.0)
}

pub fn format_entity_doc_id(id: EntityId) -> String {
    format!("entity:{}", id.0)
}

/// 将内核图路与外部（如 MemPalace traverse）候选按轮次交织合并，去重后截断。
pub fn merge_graph_rankings(
    primary: Vec<String>,
    secondary: Vec<String>,
    limit: usize,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let rounds = primary.len().max(secondary.len());
    for i in 0..rounds {
        if out.len() >= limit {
            break;
        }
        if let Some(id) = primary.get(i) {
            if seen.insert(id.clone()) {
                out.push(id.clone());
            }
        }
        if out.len() >= limit {
            break;
        }
        if let Some(id) = secondary.get(i) {
            if seen.insert(id.clone()) {
                out.push(id.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn interleaves_and_dedupes() {
        let a = vec!["e1".into(), "e2".into(), "e3".into()];
        let b = vec!["e1".into(), "x1".into()];
        let m = merge_graph_rankings(a, b, 10);
        assert_eq!(m, vec!["e1", "e2", "x1", "e3"]);
    }
}
