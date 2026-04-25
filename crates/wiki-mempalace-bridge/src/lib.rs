//! 与 `rust-mempalace` 的对接面：这里保留较细的 `MempalaceWikiSink`；更自然的集成是
//! 在引入 `wiki-kernel` 后实现其中的 `WikiHook` trait，在 `on_event` 里把 `WikiEvent` 映射到
//! Palace 的 `drawers` / `kg_facts` / 向量索引等 API。
//!
//! 启用 `live` feature 后，将使用 `rust-mempalace` 的真实实现连接 palace 数据库。
//! 默认不启用，使用 Noop 实现。

#[cfg(feature = "live")]
mod live_ranker;
#[cfg(feature = "live")]
mod live_search;
#[cfg(feature = "live")]
mod live_sink;
#[cfg(feature = "live")]
mod live_tools;

#[cfg(feature = "live")]
pub use live_ranker::LiveMempalaceGraphRanker;
#[cfg(feature = "live")]
pub use live_search::MempalaceSearchPorts;
#[cfg(feature = "live")]
pub use live_sink::LiveMempalaceSink;
#[cfg(feature = "live")]
pub use live_tools::LiveMempalaceTools;

mod tools;
pub use tools::{make_tools, MempalaceTools, NoopMempalaceTools};

use std::collections::BTreeMap;
use wiki_core::WikiEvent;
use wiki_core::{Claim, ClaimId, PageId, Scope, SourceId, WikiPage};

/// 写入外部「记忆宫殿」引擎的最小事件面（ingest / reinforce / 淘汰）。
pub trait MempalaceWikiSink: Send + Sync {
    fn on_claim_upserted(&self, claim: &Claim) -> Result<(), MempalaceError>;
    fn on_claim_event(&self, claim_id: ClaimId) -> Result<(), MempalaceError>;
    fn on_claim_superseded(&self, old: ClaimId, new: ClaimId) -> Result<(), MempalaceError>;
    fn on_source_linked(
        &self,
        source_id: SourceId,
        claim_id: ClaimId,
    ) -> Result<(), MempalaceError>;
    /// 原始资料入库（无 claim 关联时）；默认忽略。
    fn on_source_ingested(&self, _source_id: SourceId) -> Result<(), MempalaceError> {
        Ok(())
    }
    /// 页面写入后可作为 palace drawer；默认忽略。
    fn on_page_written(&self, _page: &WikiPage) -> Result<(), MempalaceError> {
        Ok(())
    }
    fn scope_filter(&self, scope: &Scope) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum MempalaceError {
    #[error("external memory backend error: {0}")]
    Backend(String),
}

/// 默认无操作，便于内核单测与不启用 mempalace 时编译通过。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopMempalace;

impl MempalaceWikiSink for NoopMempalace {
    fn on_claim_event(&self, _claim_id: ClaimId) -> Result<(), MempalaceError> {
        Ok(())
    }

    fn on_claim_superseded(&self, _old: ClaimId, _new: ClaimId) -> Result<(), MempalaceError> {
        Ok(())
    }

    fn on_claim_upserted(&self, _claim: &Claim) -> Result<(), MempalaceError> {
        Ok(())
    }

    fn on_source_linked(
        &self,
        _source_id: SourceId,
        _claim_id: ClaimId,
    ) -> Result<(), MempalaceError> {
        Ok(())
    }

    fn scope_filter(&self, _scope: &Scope) -> bool {
        true
    }
}

/// 第三路「图」召回的可插拔扩展：由宿主对接 `rust-mempalace` 的 traverse / kg_query 等。
pub trait MempalaceGraphRanker: Send + Sync {
    /// 返回 `entity:` / `claim:` 等 doc id，顺序即相关度优先。
    fn graph_rank_extras(&self, query: &str, limit: usize) -> Vec<String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopMempalaceGraphRanker;

impl MempalaceGraphRanker for NoopMempalaceGraphRanker {
    fn graph_rank_extras(&self, _query: &str, _limit: usize) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutboxEventDispatchStats {
    pub seen: usize,
    pub dispatched: usize,
    pub filtered: usize,
    pub ignored: usize,
    pub unresolved: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutboxDispatchStats {
    pub lines_seen: usize,
    pub dispatched: usize,
    pub filtered: usize,
    pub ignored: usize,
    pub unresolved: usize,
    pub by_event: BTreeMap<String, OutboxEventDispatchStats>,
}

impl OutboxDispatchStats {
    fn event_mut(&mut self, event_name: &str) -> &mut OutboxEventDispatchStats {
        self.by_event.entry(event_name.to_string()).or_default()
    }

    fn record_seen(&mut self, event_name: &str) {
        self.lines_seen += 1;
        self.event_mut(event_name).seen += 1;
    }

    fn record_dispatched(&mut self, event_name: &str) {
        self.dispatched += 1;
        self.event_mut(event_name).dispatched += 1;
    }

    fn record_filtered(&mut self, event_name: &str) {
        self.filtered += 1;
        self.event_mut(event_name).filtered += 1;
    }

    fn record_ignored(&mut self, event_name: &str) {
        self.ignored += 1;
        self.event_mut(event_name).ignored += 1;
    }

    fn record_unresolved(&mut self, event_name: &str) {
        self.unresolved += 1;
        self.event_mut(event_name).unresolved += 1;
    }
}

fn wiki_event_name(event: &WikiEvent) -> &'static str {
    match event {
        WikiEvent::SourceIngested { .. } => "SourceIngested",
        WikiEvent::ClaimUpserted { .. } => "ClaimUpserted",
        WikiEvent::ClaimSuperseded { .. } => "ClaimSuperseded",
        WikiEvent::PageWritten { .. } => "PageWritten",
        WikiEvent::QueryServed { .. } => "QueryServed",
        WikiEvent::SessionCrystallized { .. } => "SessionCrystallized",
        WikiEvent::GraphExpanded { .. } => "GraphExpanded",
        WikiEvent::LintRunFinished { .. } => "LintRunFinished",
        WikiEvent::PageStatusChanged { .. } => "PageStatusChanged",
        WikiEvent::PageDeleted { .. } => "PageDeleted",
    }
}

/// 从 id 反解回 `Claim` / 源 scope 的 resolver；由外部（通常是 wiki-storage 快照）注入。
///
/// `consume_outbox_ndjson_with_resolver` 会用它把"仅带 id"的 outbox 事件还原成完整载荷，
/// 并在此基础上执行 sink 的 scope 过滤，避免 shared outbox 跨 scope 泄漏。
pub trait OutboxResolver {
    fn claim(&self, id: ClaimId) -> Option<Claim>;
    fn source_scope(&self, id: SourceId) -> Option<Scope>;
    fn page(&self, _id: PageId) -> Option<WikiPage> {
        None
    }
    /// 默认按 `claim(id)` 取 claim 的 scope；实现可覆写以处理已删除 claim 的遗留事件。
    fn claim_scope(&self, id: ClaimId) -> Option<Scope> {
        self.claim(id).map(|c| c.scope)
    }
}

/// 历史版本：仅能还原 ID。不解析 claim 内容、不执行 scope 过滤；`ClaimUpserted` 走
/// [`MempalaceWikiSink::on_claim_event`]（在 live sink 中是 no-op），保留是为了向后兼容。
///
/// **新代码请改用 [`consume_outbox_ndjson_with_resolver`]**：它会把 `ClaimUpserted`
/// 还原为 `on_claim_upserted(&Claim)` 并尊重 [`MempalaceWikiSink::scope_filter`]。
pub fn consume_outbox_ndjson<S: MempalaceWikiSink>(
    sink: &S,
    ndjson: &str,
) -> Result<usize, MempalaceError> {
    Ok(consume_outbox_ndjson_with_stats(sink, ndjson)?.dispatched)
}

pub fn consume_outbox_ndjson_with_stats<S: MempalaceWikiSink>(
    sink: &S,
    ndjson: &str,
) -> Result<OutboxDispatchStats, MempalaceError> {
    consume_outbox_ndjson_impl::<S, NoResolver>(sink, ndjson, None)
}

/// 带 resolver 的 outbox 消费：
///
/// - `ClaimUpserted(id)` → 查 claim → `sink.scope_filter` 过滤 → `on_claim_upserted(&claim)`；
///   resolver 返回 `None` 时回退到 `on_claim_event(id)`，并**不**计入 count（视为悬挂事件）。
/// - `SourceIngested(id)` → 查 source scope → scope filter 过滤 → `on_source_ingested(id)`。
/// - `PageWritten(id)` → 查 page → scope filter 过滤 → `on_page_written(&page)`。
/// - `ClaimSuperseded { old, new }` → 如能解析 new 的 scope，经 filter 后再 `on_claim_superseded`。
///
/// 返回值为**被实际派发**（即 sink 接受）的事件数。过滤掉的事件不计入。
pub fn consume_outbox_ndjson_with_resolver<S, R>(
    sink: &S,
    resolver: &R,
    ndjson: &str,
) -> Result<usize, MempalaceError>
where
    S: MempalaceWikiSink,
    R: OutboxResolver,
{
    Ok(consume_outbox_ndjson_with_resolver_and_stats(sink, resolver, ndjson)?.dispatched)
}

pub fn consume_outbox_ndjson_with_resolver_and_stats<S, R>(
    sink: &S,
    resolver: &R,
    ndjson: &str,
) -> Result<OutboxDispatchStats, MempalaceError>
where
    S: MempalaceWikiSink,
    R: OutboxResolver,
{
    consume_outbox_ndjson_impl(sink, ndjson, Some(resolver))
}

fn consume_outbox_ndjson_impl<S, R>(
    sink: &S,
    ndjson: &str,
    resolver: Option<&R>,
) -> Result<OutboxDispatchStats, MempalaceError>
where
    S: MempalaceWikiSink,
    R: OutboxResolver,
{
    let mut stats = OutboxDispatchStats::default();
    for line in ndjson.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let event: WikiEvent = serde_json::from_str(line)
            .map_err(|e| MempalaceError::Backend(format!("invalid event json: {e}")))?;
        let event_name = wiki_event_name(&event);
        stats.record_seen(event_name);
        match event {
            WikiEvent::ClaimUpserted { claim_id, .. } => match resolver {
                Some(r) => match r.claim(claim_id) {
                    Some(claim) => {
                        if sink.scope_filter(&claim.scope) {
                            sink.on_claim_upserted(&claim)?;
                            stats.record_dispatched(event_name);
                        } else {
                            stats.record_filtered(event_name);
                        }
                    }
                    None => {
                        // 悬挂事件：claim 已被 GC 或 snapshot 落后；保持旧行为调用 on_claim_event
                        // 但不 count，避免 "consumed=N" 统计失真。
                        sink.on_claim_event(claim_id)?;
                        stats.record_unresolved(event_name);
                    }
                },
                None => {
                    sink.on_claim_event(claim_id)?;
                    stats.record_dispatched(event_name);
                }
            },
            WikiEvent::ClaimSuperseded { old, new, .. } => match resolver {
                Some(r) => match r.claim_scope(new) {
                    Some(scope) => {
                        if sink.scope_filter(&scope) {
                            sink.on_claim_superseded(old, new)?;
                            stats.record_dispatched(event_name);
                        } else {
                            stats.record_filtered(event_name);
                        }
                    }
                    None => stats.record_unresolved(event_name),
                },
                None => {
                    sink.on_claim_superseded(old, new)?;
                    stats.record_dispatched(event_name);
                }
            },
            WikiEvent::SourceIngested { source_id, .. } => {
                let allow = match resolver.and_then(|r| r.source_scope(source_id)) {
                    Some(scope) => sink.scope_filter(&scope),
                    None => true,
                };
                if allow {
                    sink.on_source_ingested(source_id)?;
                    stats.record_dispatched(event_name);
                } else {
                    stats.record_filtered(event_name);
                }
            }
            WikiEvent::PageWritten { page_id, .. } => {
                match resolver.and_then(|r| r.page(page_id)) {
                    Some(page) => {
                        if sink.scope_filter(&page.scope) {
                            sink.on_page_written(&page)?;
                            stats.record_dispatched(event_name);
                        } else {
                            stats.record_filtered(event_name);
                        }
                    }
                    None => stats.record_unresolved(event_name),
                }
            }
            _ => stats.record_ignored(event_name),
        }
    }
    Ok(stats)
}

/// 用于 `consume_outbox_ndjson` 的占位 resolver（永远返回 None）。
struct NoResolver;
impl OutboxResolver for NoResolver {
    fn claim(&self, _id: ClaimId) -> Option<Claim> {
        None
    }
    fn source_scope(&self, _id: SourceId) -> Option<Scope> {
        None
    }
    fn page(&self, _id: PageId) -> Option<WikiPage> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::{fs, path::PathBuf};
    use wiki_core::WikiEvent;

    #[derive(Clone, Default)]
    struct CountingSink {
        upserted: Arc<AtomicUsize>,
        superseded: Arc<AtomicUsize>,
        sources: Arc<AtomicUsize>,
    }

    impl MempalaceWikiSink for CountingSink {
        fn on_claim_upserted(&self, _claim: &Claim) -> Result<(), MempalaceError> {
            Ok(())
        }

        fn on_claim_event(&self, _claim_id: ClaimId) -> Result<(), MempalaceError> {
            self.upserted.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn on_claim_superseded(&self, _old: ClaimId, _new: ClaimId) -> Result<(), MempalaceError> {
            self.superseded.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn on_source_linked(
            &self,
            _source_id: SourceId,
            _claim_id: ClaimId,
        ) -> Result<(), MempalaceError> {
            Ok(())
        }

        fn scope_filter(&self, _scope: &Scope) -> bool {
            true
        }

        fn on_source_ingested(&self, _source_id: SourceId) -> Result<(), MempalaceError> {
            self.sources.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn consumes_ndjson_and_dispatches_claim_events() {
        let sink = CountingSink::default();
        let a = ClaimId(uuid::Uuid::new_v4());
        let b = ClaimId(uuid::Uuid::new_v4());

        let lines = [
            serde_json::to_string(&WikiEvent::ClaimUpserted {
                claim_id: a,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
            serde_json::to_string(&WikiEvent::ClaimSuperseded {
                old: a,
                new: b,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
        ]
        .join("\n");

        let n = consume_outbox_ndjson(&sink, &lines).unwrap();
        assert_eq!(n, 2);
        assert_eq!(sink.upserted.load(Ordering::SeqCst), 1);
        assert_eq!(sink.superseded.load(Ordering::SeqCst), 1);

        let stats = consume_outbox_ndjson_with_stats(&sink, &lines).unwrap();
        assert_eq!(stats.lines_seen, 2);
        assert_eq!(stats.dispatched, 2);
        assert_eq!(stats.ignored, 0);
        assert_eq!(stats.unresolved, 0);
        assert_eq!(stats.by_event["ClaimUpserted"].dispatched, 1);
        assert_eq!(stats.by_event["ClaimSuperseded"].dispatched, 1);
    }

    #[derive(Default)]
    struct InMemResolver {
        claims: std::collections::HashMap<ClaimId, Claim>,
        source_scopes: std::collections::HashMap<SourceId, Scope>,
        pages: std::collections::HashMap<wiki_core::PageId, wiki_core::WikiPage>,
    }
    impl OutboxResolver for InMemResolver {
        fn claim(&self, id: ClaimId) -> Option<Claim> {
            self.claims.get(&id).cloned()
        }
        fn source_scope(&self, id: SourceId) -> Option<Scope> {
            self.source_scopes.get(&id).cloned()
        }
        fn page(&self, id: wiki_core::PageId) -> Option<wiki_core::WikiPage> {
            self.pages.get(&id).cloned()
        }
    }

    #[derive(Default)]
    struct FullSink {
        upserted_ids: std::sync::Mutex<Vec<ClaimId>>,
        page_ids: std::sync::Mutex<Vec<wiki_core::PageId>>,
        filter_bank: String,
    }
    impl MempalaceWikiSink for FullSink {
        fn on_claim_upserted(&self, claim: &Claim) -> Result<(), MempalaceError> {
            self.upserted_ids.lock().unwrap().push(claim.id);
            Ok(())
        }
        fn on_claim_event(&self, _id: ClaimId) -> Result<(), MempalaceError> {
            Ok(())
        }
        fn on_claim_superseded(&self, _o: ClaimId, _n: ClaimId) -> Result<(), MempalaceError> {
            Ok(())
        }
        fn on_source_linked(&self, _s: SourceId, _c: ClaimId) -> Result<(), MempalaceError> {
            Ok(())
        }
        fn on_page_written(&self, page: &wiki_core::WikiPage) -> Result<(), MempalaceError> {
            self.page_ids.lock().unwrap().push(page.id);
            Ok(())
        }
        fn scope_filter(&self, scope: &Scope) -> bool {
            match scope {
                Scope::Private { agent_id } => agent_id == &self.filter_bank,
                Scope::Shared { team_id } => team_id == &self.filter_bank,
            }
        }
    }

    fn mk_claim(id: ClaimId, text: &str, scope: Scope) -> Claim {
        let mut c = Claim::new(text, scope, wiki_core::MemoryTier::Semantic);
        c.id = id;
        c
    }

    #[test]
    fn resolver_path_materializes_claim_and_enforces_scope() {
        use wiki_core::Scope;
        let a = ClaimId(uuid::Uuid::new_v4()); // private:alice —— 被过滤
        let b = ClaimId(uuid::Uuid::new_v4()); // private:bob   —— 被保留
        let mut resolver = InMemResolver::default();
        resolver.claims.insert(
            a,
            mk_claim(
                a,
                "private to alice",
                Scope::Private {
                    agent_id: "alice".into(),
                },
            ),
        );
        resolver.claims.insert(
            b,
            mk_claim(
                b,
                "private to bob",
                Scope::Private {
                    agent_id: "bob".into(),
                },
            ),
        );

        let sink = FullSink {
            filter_bank: "bob".into(),
            ..Default::default()
        };
        let lines = [
            serde_json::to_string(&WikiEvent::ClaimUpserted {
                claim_id: a,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
            serde_json::to_string(&WikiEvent::ClaimUpserted {
                claim_id: b,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
        ]
        .join("\n");

        let n = consume_outbox_ndjson_with_resolver(&sink, &resolver, &lines).unwrap();
        assert_eq!(n, 1, "仅 bob 的 claim 应被派发");
        let got = sink.upserted_ids.lock().unwrap().clone();
        assert_eq!(got, vec![b]);

        let stats =
            consume_outbox_ndjson_with_resolver_and_stats(&sink, &resolver, &lines).unwrap();
        assert_eq!(stats.lines_seen, 2);
        assert_eq!(stats.dispatched, 1);
        assert_eq!(stats.filtered, 1);
        assert_eq!(stats.unresolved, 0);
        assert_eq!(stats.by_event["ClaimUpserted"].seen, 2);
        assert_eq!(stats.by_event["ClaimUpserted"].dispatched, 1);
        assert_eq!(stats.by_event["ClaimUpserted"].filtered, 1);
    }

    #[test]
    fn resolver_path_materializes_page_written_and_enforces_scope() {
        use wiki_core::{Scope, WikiPage};

        let allowed_page = WikiPage::new(
            "Shared Page",
            "# Shared Page\n\nbody",
            Scope::Shared {
                team_id: "wiki".into(),
            },
        );
        let filtered_page = WikiPage::new(
            "Other Page",
            "# Other Page\n\nbody",
            Scope::Shared {
                team_id: "other".into(),
            },
        );
        let allowed_id = allowed_page.id;
        let filtered_id = filtered_page.id;

        let mut resolver = InMemResolver::default();
        resolver.pages.insert(allowed_id, allowed_page);
        resolver.pages.insert(filtered_id, filtered_page);

        let sink = FullSink {
            filter_bank: "wiki".into(),
            ..Default::default()
        };
        let lines = [
            serde_json::to_string(&WikiEvent::PageWritten {
                page_id: allowed_id,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
            serde_json::to_string(&WikiEvent::PageWritten {
                page_id: filtered_id,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
        ]
        .join("\n");

        let stats =
            consume_outbox_ndjson_with_resolver_and_stats(&sink, &resolver, &lines).unwrap();
        assert_eq!(stats.lines_seen, 2);
        assert_eq!(stats.dispatched, 1);
        assert_eq!(stats.filtered, 1);
        assert_eq!(stats.unresolved, 0);
        assert_eq!(stats.by_event["PageWritten"].dispatched, 1);
        assert_eq!(stats.by_event["PageWritten"].filtered, 1);
        assert_eq!(*sink.page_ids.lock().unwrap(), vec![allowed_id]);
    }

    #[test]
    fn unresolved_supersede_scope_is_not_dispatched() {
        let old = ClaimId(uuid::Uuid::new_v4());
        let new = ClaimId(uuid::Uuid::new_v4());
        let sink = CountingSink::default();
        let resolver = InMemResolver::default();
        let line = serde_json::to_string(&WikiEvent::ClaimSuperseded {
            old,
            new,
            at: time::OffsetDateTime::now_utc(),
        })
        .unwrap();

        let stats = consume_outbox_ndjson_with_resolver_and_stats(&sink, &resolver, &line).unwrap();

        assert_eq!(stats.dispatched, 0);
        assert_eq!(stats.unresolved, 1);
        assert_eq!(sink.superseded.load(Ordering::SeqCst), 0);
        assert_eq!(stats.by_event["ClaimSuperseded"].unresolved, 1);
    }

    #[test]
    fn consumes_source_ingested() {
        let sink = CountingSink {
            upserted: Arc::new(AtomicUsize::new(0)),
            superseded: Arc::new(AtomicUsize::new(0)),
            sources: Arc::new(AtomicUsize::new(0)),
        };
        let sid = SourceId(uuid::Uuid::new_v4());
        let line = serde_json::to_string(&WikiEvent::SourceIngested {
            source_id: sid,
            redacted: false,
            at: time::OffsetDateTime::now_utc(),
        })
        .unwrap();
        let n = consume_outbox_ndjson(&sink, &line).unwrap();
        assert_eq!(n, 1);
        assert_eq!(sink.sources.load(Ordering::SeqCst), 1);

        let stats = consume_outbox_ndjson_with_stats(&sink, &line).unwrap();
        assert_eq!(stats.lines_seen, 1);
        assert_eq!(stats.dispatched, 1);
        assert_eq!(stats.by_event["SourceIngested"].dispatched, 1);
    }

    #[test]
    fn stats_mark_unresolved_and_ignored_events() {
        let sink = CountingSink::default();
        let missing_claim = ClaimId(uuid::Uuid::new_v4());
        let lines = [
            serde_json::to_string(&WikiEvent::ClaimUpserted {
                claim_id: missing_claim,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
            serde_json::to_string(&WikiEvent::QueryServed {
                query_fingerprint: "q".into(),
                top_doc_ids: vec!["claim:1".into()],
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
            serde_json::to_string(&WikiEvent::LintRunFinished {
                findings: 2,
                at: time::OffsetDateTime::now_utc(),
            })
            .unwrap(),
        ]
        .join("\n");

        let resolver = InMemResolver::default();
        let stats =
            consume_outbox_ndjson_with_resolver_and_stats(&sink, &resolver, &lines).unwrap();
        assert_eq!(stats.lines_seen, 3);
        assert_eq!(stats.dispatched, 0);
        assert_eq!(stats.unresolved, 1);
        assert_eq!(stats.ignored, 2);
        assert_eq!(sink.upserted.load(Ordering::SeqCst), 1);
        assert_eq!(stats.by_event["ClaimUpserted"].unresolved, 1);
        assert_eq!(stats.by_event["QueryServed"].ignored, 1);
        assert_eq!(stats.by_event["LintRunFinished"].ignored, 1);
    }

    #[test]
    fn event_matrix_doc_stays_in_sync_with_wiki_event_variants() {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let event_source =
            fs::read_to_string(workspace_root.join("crates/wiki-core/src/events.rs")).unwrap();
        let documented =
            fs::read_to_string(workspace_root.join("docs/outbox-event-matrix.md")).unwrap();

        let actual_events: Vec<String> = event_source
            .lines()
            .map(str::trim)
            .filter(|line| line.ends_with('{') && !line.starts_with("pub enum"))
            .map(|line| line.trim_end_matches('{').trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        let documented_events: Vec<String> = documented
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if !trimmed.starts_with("| `") {
                    return None;
                }
                trimmed.split('`').nth(1).map(|s| s.to_string())
            })
            .collect();

        assert_eq!(documented_events, actual_events);
        assert!(documented.contains("| `PageWritten` |"));
        assert!(
            documented.contains("| `PageWritten` | defined-not-emitted |")
                || documented.contains("`PageWritten`")
        );
        assert!(documented.contains("| `GraphExpanded` |"));
    }
}
