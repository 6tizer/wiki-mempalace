//! Gap 扫描逻辑：检测知识库中的缺口。
//!
//! 三类 gap 规则：
//! - `gap.missing_xref`：claim 的关键词没有被任何 page 引用
//! - `gap.low_coverage`：entity 只有极少量 claim，覆盖不足
//! - `gap.orphan_source`：source 没有对应的 summary/concept page

use wiki_core::{GapFinding, Scope};

use crate::InMemoryStore;

/// 对整个知识库运行 gap 扫描，返回所有检测到的缺口。
///
/// `viewer_scope` 用于过滤：只检测对 viewer 可见的文档。
/// `low_coverage_threshold` 定义"低覆盖"的 claim 数量阈值（默认 2）。
pub fn run_gap_scan(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
    low_coverage_threshold: usize,
) -> Vec<GapFinding> {
    let mut findings = Vec::new();

    findings.extend(scan_missing_xref(store, viewer_scope));
    findings.extend(scan_low_coverage(store, viewer_scope, low_coverage_threshold));
    findings.extend(scan_orphan_source(store, viewer_scope));

    findings
}

/// `gap.missing_xref`：claim 的关键词没有被任何 page 引用。
///
/// 逻辑：对每个非 stale 的 claim，提取其关键词（>=4 字符的词），
/// 如果没有任何 page 的 markdown 包含这些关键词，则报告缺口。
fn scan_missing_xref(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GapFinding> {
    // TODO: 子代理 A 实现
    let _ = (store, viewer_scope);
    Vec::new()
}

/// `gap.low_coverage`：entity 只有极少量 claim，覆盖不足。
///
/// 逻辑：统计每个 entity 关联的 claim 数量。如果数量低于阈值，
/// 说明该主题的覆盖不充分。
fn scan_low_coverage(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
    _threshold: usize,
) -> Vec<GapFinding> {
    // TODO: 子代理 A 实现
    let _ = (store, viewer_scope);
    Vec::new()
}

/// `gap.orphan_source`：source 没有对应的 summary/concept page。
///
/// 逻辑：检查每个 source 是否至少被一个 page（通过 claim 引用链）
/// 关联。如果一个 source 的 claim 没有任何出现在 page markdown 中，
/// 则该 source 是孤立的。
fn scan_orphan_source(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GapFinding> {
    // TODO: 子代理 A 实现
    let _ = (store, viewer_scope);
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{MemoryTier, Scope};

    // TODO: 子代理 A 补充单元测试

    #[test]
    fn gap_scan_empty_store_no_panic() {
        let store = InMemoryStore::default();
        let findings = run_gap_scan(&store, None, 2);
        assert!(findings.is_empty());
    }
}

