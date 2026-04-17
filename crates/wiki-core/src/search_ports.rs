/// 混合检索的三路有序 doc id（约定：`claim:`、`page:`、`entity:` 前缀）。
pub trait SearchPorts {
    fn bm25_ranked_ids(&self, query: &str, limit: usize) -> Vec<String>;
    fn vector_ranked_ids(&self, query: &str, limit: usize) -> Vec<String>;
    fn graph_ranked_ids(&self, query: &str, limit: usize) -> Vec<String>;
}
