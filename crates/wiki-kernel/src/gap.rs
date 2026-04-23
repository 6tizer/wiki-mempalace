//! Gap 扫描逻辑：检测知识库中的缺口。
//!
//! 三类 gap 规则：
//! - `gap.missing_xref`：claim 的关键词没有被任何 page 引用
//! - `gap.low_coverage`：entity 只有极少量 claim，覆盖不足
//! - `gap.orphan_source`：source 没有对应的 summary/concept page

use wiki_core::{document_visible_to_viewer, GapFinding, GapSeverity, Scope};

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
    findings.extend(scan_low_coverage(
        store,
        viewer_scope,
        low_coverage_threshold,
    ));
    findings.extend(scan_orphan_source(store, viewer_scope));

    findings
}

/// `gap.missing_xref`：claim 的关键词没有被任何 page 引用。
///
/// 逻辑：对每个非 stale 的 claim，提取其关键词（>=4 字符的词），
/// 如果没有任何 page 的 markdown 包含这些关键词，则报告缺口。
fn scan_missing_xref(store: &InMemoryStore, viewer_scope: Option<&Scope>) -> Vec<GapFinding> {
    let visible = |s: &Scope| match viewer_scope {
        None => true,
        Some(v) => document_visible_to_viewer(s, v),
    };

    // 收集所有可见 page 的 markdown 文本（转小写）
    let mut page_text = String::new();
    for p in store.pages.values() {
        if visible(&p.scope) {
            page_text.push_str(&p.markdown.to_ascii_lowercase());
            page_text.push('\n');
        }
    }

    let mut findings = Vec::new();
    for c in store.claims.values() {
        if !visible(&c.scope) {
            continue;
        }
        if c.stale {
            continue;
        }
        if !claim_has_page_reference(&c.text, &page_text) {
            findings.push(GapFinding {
                code: "gap.missing_xref".into(),
                message: format!(
                    "claim 关键词未被任何 page 引用: {}",
                    c.text.chars().take(60).collect::<String>()
                ),
                severity: GapSeverity::Medium,
                subject: Some(c.id.0.to_string()),
                subject_label: Some(c.text.chars().take(60).collect::<String>()),
            });
        }
    }
    findings
}

/// `gap.low_coverage`：entity 只有极少量 claim，覆盖不足。
///
/// 逻辑：统计每个 entity 关联的 claim 数量（通过 claim 文本中包含 entity label 匹配）。
/// 如果数量低于阈值，说明该主题的覆盖不充分。
fn scan_low_coverage(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
    threshold: usize,
) -> Vec<GapFinding> {
    let visible = |s: &Scope| match viewer_scope {
        None => true,
        Some(v) => document_visible_to_viewer(s, v),
    };

    // 统计每个可见 entity 关联的可见 claim 数量
    let mut entity_claim_counts: std::collections::HashMap<_, usize> =
        std::collections::HashMap::new();

    for e in store.entities.values() {
        if !visible(&e.scope) {
            continue;
        }
        let label_lower = e.label.to_ascii_lowercase();
        let count = store
            .claims
            .values()
            .filter(|c| {
                visible(&c.scope) && !c.stale && c.text.to_ascii_lowercase().contains(&label_lower)
            })
            .count();
        entity_claim_counts.insert(e.id, count);
    }

    let mut findings = Vec::new();
    for e in store.entities.values() {
        if !visible(&e.scope) {
            continue;
        }
        let count = entity_claim_counts.get(&e.id).copied().unwrap_or(0);
        if count < threshold {
            let severity = match count {
                0 => GapSeverity::High,
                1 => GapSeverity::Medium,
                _ => GapSeverity::Low,
            };
            findings.push(GapFinding {
                code: "gap.low_coverage".into(),
                message: format!(
                    "entity '{}' 仅有 {} 条 claim，低于阈值 {}",
                    e.label, count, threshold
                ),
                severity,
                subject: Some(e.id.0.to_string()),
                subject_label: Some(e.label.clone()),
            });
        }
    }
    findings
}

/// `gap.orphan_source`：source 没有对应的 summary/concept page。
///
/// 逻辑：检查每个 source 是否至少被一个 page（通过 claim 引用链）
/// 关联。如果一个 source 的所有 claim 的关键词都没有出现在任何 page markdown 中，
/// 则该 source 是孤立的。
fn scan_orphan_source(store: &InMemoryStore, viewer_scope: Option<&Scope>) -> Vec<GapFinding> {
    let visible = |s: &Scope| match viewer_scope {
        None => true,
        Some(v) => document_visible_to_viewer(s, v),
    };

    // 收集所有可见 page 的 markdown 文本（转小写）
    let mut page_text = String::new();
    for p in store.pages.values() {
        if visible(&p.scope) {
            page_text.push_str(&p.markdown.to_ascii_lowercase());
            page_text.push('\n');
        }
    }

    // 预先建立 source -> claims 的反向索引（仅可见 claim）
    let mut source_to_claims: std::collections::HashMap<_, Vec<_>> =
        std::collections::HashMap::new();
    for c in store.claims.values() {
        if !visible(&c.scope) || c.stale {
            continue;
        }
        for sid in &c.source_ids {
            source_to_claims.entry(*sid).or_default().push(c);
        }
    }

    let mut findings = Vec::new();
    for s in store.sources.values() {
        if !visible(&s.scope) {
            continue;
        }

        let related_claims = source_to_claims.get(&s.id).cloned().unwrap_or_default();

        // 如果 source 没有任何可见 claim，也算 orphan
        let has_page_ref = if related_claims.is_empty() {
            false
        } else {
            related_claims
                .iter()
                .any(|c| claim_has_page_reference(&c.text, &page_text))
        };

        if !has_page_ref {
            let msg = if related_claims.is_empty() {
                format!(
                    "source '{}' 无关联 claim，尚未被知识库消化",
                    s.uri.chars().take(60).collect::<String>()
                )
            } else {
                format!(
                    "source '{}' 的 claim 未被任何 page 引用",
                    s.uri.chars().take(60).collect::<String>()
                )
            };
            findings.push(GapFinding {
                code: "gap.orphan_source".into(),
                message: msg,
                severity: GapSeverity::High,
                subject: Some(s.id.0.to_string()),
                subject_label: Some(s.uri.clone()),
            });
        }
    }
    findings
}

/// 判断 claim 的文本关键词是否至少有一个出现在 page 文本中。
///
/// 共享函数：engine.rs 的 run_basic_lint 和 gap.rs 的扫描规则都依赖它。
/// 保持单一定义以避免关键词提取策略漂移。
pub fn claim_has_page_reference(claim_text: &str, page_text: &str) -> bool {
    let keys: Vec<String> = claim_text
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|x| x.trim().to_ascii_lowercase())
        .filter(|x| x.len() >= 4)
        .collect();
    if keys.is_empty() {
        return true;
    }
    keys.iter().any(|k| page_text.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use wiki_core::{
        Claim, Entity, EntityId, EntityKind, MemoryTier, RawArtifact, Scope, WikiPage,
    };

    fn private_scope(agent_id: &str) -> Scope {
        Scope::Private {
            agent_id: agent_id.into(),
        }
    }

    #[test]
    fn gap_scan_empty_store_no_panic() {
        let store = InMemoryStore::default();
        let findings = run_gap_scan(&store, None, 2);
        assert!(findings.is_empty());
    }

    #[test]
    fn missing_xref_triggered() {
        let mut store = InMemoryStore::default();
        let claim = Claim::new(
            "项目使用 Redis 进行缓存",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        store.claims.insert(claim.id, claim);

        let findings = scan_missing_xref(&store, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "gap.missing_xref");
    }

    #[test]
    fn missing_xref_not_triggered() {
        let mut store = InMemoryStore::default();
        let claim = Claim::new(
            "项目使用 Redis 进行缓存",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        store.claims.insert(claim.id, claim.clone());

        let page = WikiPage::new(
            "技术方案",
            "我们的项目使用 Redis 进行缓存。",
            private_scope("a"),
        );
        store.pages.insert(page.id, page);

        let findings = scan_missing_xref(&store, None);
        assert!(
            findings.is_empty(),
            "claim 关键词已在 page 中，不应触发 gap"
        );
    }

    #[test]
    fn low_coverage_entity() {
        let mut store = InMemoryStore::default();
        let entity = Entity {
            id: EntityId(Uuid::new_v4()),
            kind: EntityKind::Project,
            label: "AlphaProject".into(),
            scope: private_scope("a"),
        };
        store.entities.insert(entity.id, entity.clone());

        // 0 个 claim → High
        let findings = scan_low_coverage(&store, None, 2);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "gap.low_coverage");
        assert_eq!(findings[0].severity, GapSeverity::High);

        // 添加 1 个 claim → Medium（仍低于阈值 2）
        let claim = Claim::new(
            "AlphaProject 使用 Rust 开发",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        store.claims.insert(claim.id, claim);

        let findings = scan_low_coverage(&store, None, 2);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, GapSeverity::Medium);

        // 添加第 2 个 claim → 达到阈值，不再报告
        let claim2 = Claim::new(
            "AlphaProject 依赖 tokio",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        store.claims.insert(claim2.id, claim2);

        let findings = scan_low_coverage(&store, None, 2);
        assert!(findings.is_empty(), "claim 数量达到阈值，不应触发 gap");
    }

    #[test]
    fn stale_claims_do_not_count_toward_low_coverage() {
        let mut store = InMemoryStore::default();
        let entity = Entity {
            id: EntityId(Uuid::new_v4()),
            kind: EntityKind::Project,
            label: "AlphaProject".into(),
            scope: private_scope("a"),
        };
        store.entities.insert(entity.id, entity.clone());

        let mut stale_claim = Claim::new(
            "AlphaProject 使用 Rust 开发",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        stale_claim.stale = true;
        store.claims.insert(stale_claim.id, stale_claim);

        let findings = scan_low_coverage(&store, None, 2);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "gap.low_coverage");
        assert_eq!(findings[0].severity, GapSeverity::High);
    }

    #[test]
    fn orphan_source_detected() {
        let mut store = InMemoryStore::default();
        let source = RawArtifact::new("file:///tmp/note.md", "some body", private_scope("a"));
        let sid = source.id;
        store.sources.insert(sid, source);

        let claim = Claim::new(
            "项目使用 Redis 进行缓存",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        let mut claim_with_source = claim.clone();
        claim_with_source.source_ids.push(sid);
        store.claims.insert(claim.id, claim_with_source);

        let findings = scan_orphan_source(&store, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "gap.orphan_source");
        assert_eq!(findings[0].severity, GapSeverity::High);
    }

    #[test]
    fn orphan_source_with_page_not_detected() {
        let mut store = InMemoryStore::default();
        let source = RawArtifact::new("file:///tmp/note.md", "some body", private_scope("a"));
        let sid = source.id;
        store.sources.insert(sid, source);

        let claim = Claim::new(
            "项目使用 Redis 进行缓存",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        let mut claim_with_source = claim.clone();
        claim_with_source.source_ids.push(sid);
        store.claims.insert(claim.id, claim_with_source);

        let page = WikiPage::new(
            "技术方案",
            "我们的项目使用 Redis 进行缓存。",
            private_scope("a"),
        );
        store.pages.insert(page.id, page);

        let findings = scan_orphan_source(&store, None);
        assert!(
            findings.is_empty(),
            "source 的 claim 已被 page 引用，不应触发 gap"
        );
    }

    #[test]
    fn stale_claims_do_not_prevent_orphan_source_detection() {
        let mut store = InMemoryStore::default();
        let source = RawArtifact::new("file:///tmp/note.md", "some body", private_scope("a"));
        let sid = source.id;
        store.sources.insert(sid, source);

        let mut claim = Claim::new(
            "项目使用 Redis 进行缓存",
            private_scope("a"),
            MemoryTier::Semantic,
        );
        claim.source_ids.push(sid);
        claim.stale = true;
        store.claims.insert(claim.id, claim);

        let page = WikiPage::new(
            "技术方案",
            "我们的项目使用 Redis 进行缓存。",
            private_scope("a"),
        );
        store.pages.insert(page.id, page);

        let findings = scan_orphan_source(&store, None);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "gap.orphan_source");
    }
}
