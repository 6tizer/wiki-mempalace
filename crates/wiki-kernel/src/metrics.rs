use std::collections::HashMap;

use time::OffsetDateTime;
use wiki_core::{
    document_visible_to_viewer, ContentMetrics, DomainSchema, EntryStatus, EntryType, GapMetrics,
    LifecycleMetrics, OutboxMetrics, Scope, WikiMetricsReport,
};
use wiki_storage::{OutboxConsumerProgress, OutboxStats};

use crate::{collect_basic_lint_findings, run_gap_scan, InMemoryStore};

pub fn collect_wiki_metrics(
    store: &InMemoryStore,
    schema: &DomainSchema,
    viewer_scope: Option<&Scope>,
    outbox_stats: Option<&OutboxStats>,
    outbox_progress: Option<&OutboxConsumerProgress>,
    low_coverage_threshold: usize,
    generated_at: OffsetDateTime,
) -> WikiMetricsReport {
    let mut report = WikiMetricsReport::new(generated_at);
    report.content = collect_content_metrics(store, viewer_scope);
    report.lint = collect_basic_lint_findings(schema, store, viewer_scope)
        .into_iter()
        .fold(Default::default(), |mut acc, finding| {
            acc.add_severity(finding.severity);
            acc
        });
    report.gaps = run_gap_scan(store, viewer_scope, low_coverage_threshold)
        .into_iter()
        .fold(GapMetrics::default(), |mut acc, finding| {
            acc.add_severity(finding.severity);
            acc
        });
    report.outbox = collect_outbox_metrics(outbox_stats, outbox_progress);
    report.lifecycle = collect_lifecycle_metrics(store, viewer_scope);
    report
}

fn collect_content_metrics(store: &InMemoryStore, viewer_scope: Option<&Scope>) -> ContentMetrics {
    let sources = store
        .sources
        .values()
        .filter(|source| visible(&source.scope, viewer_scope))
        .count() as u64;
    let pages = store
        .pages
        .values()
        .filter(|page| visible(&page.scope, viewer_scope))
        .count() as u64;
    let claims = store
        .claims
        .values()
        .filter(|claim| visible(&claim.scope, viewer_scope))
        .count() as u64;
    let entities = store
        .entities
        .values()
        .filter(|entity| visible(&entity.scope, viewer_scope))
        .count() as u64;
    let relations = store
        .edges
        .iter()
        .filter(|edge| {
            let Some(from) = store.entities.get(&edge.from) else {
                return false;
            };
            let Some(to) = store.entities.get(&edge.to) else {
                return false;
            };
            visible(&from.scope, viewer_scope) && visible(&to.scope, viewer_scope)
        })
        .count() as u64;

    ContentMetrics {
        sources,
        pages,
        claims,
        entities,
        relations,
    }
}

fn collect_outbox_metrics(
    stats: Option<&OutboxStats>,
    progress: Option<&OutboxConsumerProgress>,
) -> OutboxMetrics {
    let Some(stats) = stats else {
        return OutboxMetrics::default();
    };

    OutboxMetrics {
        head_id: (stats.head_id > 0).then_some(stats.head_id),
        total_events: non_negative_i64_to_u64(stats.total_events),
        unprocessed_events: non_negative_i64_to_u64(stats.unprocessed_events),
        consumer_tag: progress.map(|p| p.consumer_tag.clone()),
        acked_up_to_id: progress.and_then(|p| p.acked_up_to_id),
        backlog_events: progress
            .map(|p| non_negative_i64_to_u64(p.backlog_events))
            .unwrap_or_else(|| non_negative_i64_to_u64(stats.unprocessed_events)),
    }
}

fn collect_lifecycle_metrics(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> LifecycleMetrics {
    let mut status_counts: HashMap<EntryStatus, u64> = HashMap::new();
    let mut type_counts: HashMap<EntryType, u64> = HashMap::new();

    for page in store.pages.values() {
        if !visible(&page.scope, viewer_scope) {
            continue;
        }
        *status_counts.entry(page.status).or_insert(0) += 1;
        if let Some(entry_type) = &page.entry_type {
            *type_counts.entry(entry_type.clone()).or_insert(0) += 1;
        }
    }

    let stale_claims = store
        .claims
        .values()
        .filter(|claim| visible(&claim.scope, viewer_scope) && claim.stale)
        .count() as u64;

    let mut lifecycle = LifecycleMetrics {
        stale_claims,
        ..Default::default()
    };
    for status in [
        EntryStatus::Draft,
        EntryStatus::InReview,
        EntryStatus::Approved,
        EntryStatus::NeedsUpdate,
    ] {
        lifecycle.add_page_status(status, status_counts.remove(&status).unwrap_or(0));
    }
    for entry_type in [
        EntryType::Concept,
        EntryType::Entity,
        EntryType::Summary,
        EntryType::Synthesis,
        EntryType::Qa,
        EntryType::LintReport,
        EntryType::Index,
    ] {
        lifecycle.add_entry_type(
            entry_type.clone(),
            type_counts.remove(&entry_type).unwrap_or(0),
        );
    }
    lifecycle
}

fn visible(scope: &Scope, viewer_scope: Option<&Scope>) -> bool {
    match viewer_scope {
        None => true,
        Some(viewer) => document_visible_to_viewer(scope, viewer),
    }
}

fn non_negative_i64_to_u64(value: i64) -> u64 {
    value.max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;
    use wiki_core::{
        Claim, Entity, EntityId, EntityKind, MemoryTier, RawArtifact, RelationKind, TypedEdge,
        WikiPage,
    };
    use wiki_storage::{OutboxConsumerProgress, OutboxStats};

    fn private_scope(agent_id: &str) -> Scope {
        Scope::Private {
            agent_id: agent_id.to_string(),
        }
    }

    fn generated_at() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn empty_store_metrics_are_zero() {
        let store = InMemoryStore::default();
        let report = collect_wiki_metrics(
            &store,
            &DomainSchema::permissive_default(),
            None,
            None,
            None,
            2,
            generated_at(),
        );

        assert_eq!(report.content, ContentMetrics::default());
        assert_eq!(report.lint.total_findings, 0);
        assert_eq!(report.gaps.total_findings, 0);
        assert_eq!(report.outbox, OutboxMetrics::default());
        assert_eq!(report.lifecycle.stale_claims, 0);
        assert!(report
            .lifecycle
            .page_status
            .iter()
            .all(|item| item.count == 0));
        assert!(report
            .lifecycle
            .entry_type
            .iter()
            .all(|item| item.count == 0));
    }

    #[test]
    fn viewer_scope_filters_private_content_and_relations() {
        let mut store = InMemoryStore::default();
        let a = private_scope("a");
        let b = private_scope("b");

        let source_a = RawArtifact::new("file:///a.md", "body", a.clone());
        let source_b = RawArtifact::new("file:///b.md", "body", b.clone());
        store.sources.insert(source_a.id, source_a);
        store.sources.insert(source_b.id, source_b);

        let claim_a = Claim::new("alpha claim", a.clone(), MemoryTier::Semantic);
        let claim_b = Claim::new("beta claim", b.clone(), MemoryTier::Semantic);
        store.claims.insert(claim_a.id, claim_a);
        store.claims.insert(claim_b.id, claim_b);

        let page_a = WikiPage::new("Alpha", "alpha claim", a.clone());
        let page_b = WikiPage::new("Beta", "beta claim", b.clone());
        store.pages.insert(page_a.id, page_a);
        store.pages.insert(page_b.id, page_b);

        let entity_a = entity("Alpha", a.clone());
        let entity_b = entity("Beta", b.clone());
        let entity_a2 = entity("Alpha2", a.clone());
        store.entities.insert(entity_a.id, entity_a.clone());
        store.entities.insert(entity_b.id, entity_b.clone());
        store.entities.insert(entity_a2.id, entity_a2.clone());
        store.edges.push(edge(entity_a.id, entity_a2.id));
        store.edges.push(edge(entity_a.id, entity_b.id));

        let report = collect_wiki_metrics(
            &store,
            &DomainSchema::permissive_default(),
            Some(&a),
            None,
            None,
            2,
            generated_at(),
        );

        assert_eq!(report.content.sources, 1);
        assert_eq!(report.content.pages, 1);
        assert_eq!(report.content.claims, 1);
        assert_eq!(report.content.entities, 2);
        assert_eq!(report.content.relations, 1);
    }

    #[test]
    fn lifecycle_counts_visible_status_type_and_stale_claims() {
        let mut store = InMemoryStore::default();
        let a = private_scope("a");
        let b = private_scope("b");

        let approved = WikiPage::new("Approved", "body", a.clone())
            .with_entry_type(EntryType::Concept)
            .with_status(EntryStatus::Approved);
        let draft = WikiPage::new("Draft", "body", a.clone()).with_entry_type(EntryType::Summary);
        let hidden = WikiPage::new("Hidden", "body", b.clone())
            .with_entry_type(EntryType::Entity)
            .with_status(EntryStatus::NeedsUpdate);
        store.pages.insert(approved.id, approved);
        store.pages.insert(draft.id, draft);
        store.pages.insert(hidden.id, hidden);

        let mut stale_visible = Claim::new("old alpha claim", a.clone(), MemoryTier::Semantic);
        stale_visible.stale = true;
        let mut stale_hidden = Claim::new("old beta claim", b.clone(), MemoryTier::Semantic);
        stale_hidden.stale = true;
        store.claims.insert(stale_visible.id, stale_visible);
        store.claims.insert(stale_hidden.id, stale_hidden);

        let report = collect_wiki_metrics(
            &store,
            &DomainSchema::permissive_default(),
            Some(&a),
            None,
            None,
            2,
            generated_at(),
        );

        assert_eq!(status_count(&report.lifecycle, EntryStatus::Draft), 1);
        assert_eq!(status_count(&report.lifecycle, EntryStatus::Approved), 1);
        assert_eq!(status_count(&report.lifecycle, EntryStatus::NeedsUpdate), 0);
        assert_eq!(type_count(&report.lifecycle, EntryType::Concept), 1);
        assert_eq!(type_count(&report.lifecycle, EntryType::Summary), 1);
        assert_eq!(type_count(&report.lifecycle, EntryType::Entity), 0);
        assert_eq!(report.lifecycle.stale_claims, 1);
    }

    #[test]
    fn outbox_stats_and_progress_are_mapped() {
        let stats = OutboxStats {
            head_id: 42,
            total_events: 10,
            unprocessed_events: 4,
        };
        let progress = OutboxConsumerProgress {
            consumer_tag: "mempalace".to_string(),
            acked_up_to_id: Some(38),
            acked_at: Some(generated_at()),
            backlog_events: 4,
        };

        let report = collect_wiki_metrics(
            &InMemoryStore::default(),
            &DomainSchema::permissive_default(),
            None,
            Some(&stats),
            Some(&progress),
            2,
            generated_at(),
        );

        assert_eq!(report.outbox.head_id, Some(42));
        assert_eq!(report.outbox.total_events, 10);
        assert_eq!(report.outbox.unprocessed_events, 4);
        assert_eq!(report.outbox.consumer_tag.as_deref(), Some("mempalace"));
        assert_eq!(report.outbox.acked_up_to_id, Some(38));
        assert_eq!(report.outbox.backlog_events, 4);
    }

    fn entity(label: &str, scope: Scope) -> Entity {
        Entity {
            id: EntityId(uuid::Uuid::new_v4()),
            kind: EntityKind::Concept,
            label: label.to_string(),
            scope,
        }
    }

    fn edge(from: EntityId, to: EntityId) -> TypedEdge {
        TypedEdge {
            from,
            to,
            relation: RelationKind::Related,
            confidence: 0.9,
            source_ids: Vec::new(),
        }
    }

    fn status_count(metrics: &LifecycleMetrics, status: EntryStatus) -> u64 {
        metrics
            .page_status
            .iter()
            .find(|item| item.status == status)
            .map(|item| item.count)
            .unwrap_or(0)
    }

    fn type_count(metrics: &LifecycleMetrics, entry_type: EntryType) -> u64 {
        metrics
            .entry_type
            .iter()
            .find(|item| item.entry_type == entry_type)
            .map(|item| item.count)
            .unwrap_or(0)
    }
}
