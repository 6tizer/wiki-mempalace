use std::collections::BTreeMap;

use time::OffsetDateTime;
use wiki_core::{
    document_visible_to_viewer, DomainSchema, EntryStatus, FixActionType, Scope,
    StrategyExecutionPolicy, StrategyReport, StrategySeverity, StrategySuggestion, WikiEvent,
    WikiMetricsReport,
};

use crate::{collect_basic_lint_findings, map_findings_to_fixes, run_gap_scan, InMemoryStore};

pub struct StrategyScanOptions<'a> {
    pub viewer_scope: Option<&'a Scope>,
    pub low_coverage_threshold: usize,
    pub generated_at: OffsetDateTime,
    pub report_id: String,
}

pub fn run_strategy_scan(
    store: &InMemoryStore,
    schema: &DomainSchema,
    metrics: &WikiMetricsReport,
    query_events: &[WikiEvent],
    options: StrategyScanOptions<'_>,
) -> StrategyReport {
    let report_id = options.report_id;
    let viewer_scope = options.viewer_scope;
    let mut builder = StrategyReportBuilder::new(report_id.clone());

    let lint_findings = collect_basic_lint_findings(schema, store, viewer_scope);
    let gap_findings = run_gap_scan(store, viewer_scope, options.low_coverage_threshold);
    for fix in map_findings_to_fixes(&lint_findings, &gap_findings) {
        let mut suggestion = match fix.fix_type {
            FixActionType::Auto => builder.suggestion(
                "suggest.fix_auto_safe",
                StrategySeverity::Low,
                format!("Low-risk fixer action is available: {}", fix.description),
                StrategyExecutionPolicy::AutoSafe,
            ),
            FixActionType::Draft => builder.suggestion(
                "suggest.stale_review",
                StrategySeverity::Medium,
                format!("Draft fixer action needs agent review: {}", fix.description),
                StrategyExecutionPolicy::AgentReview,
            ),
            FixActionType::Manual => {
                if fix.code == "claim.stale" || fix.code == "lifecycle.stale" {
                    continue;
                }
                let policy = manual_fix_execution_policy(&fix.code);
                builder.suggestion(
                    "suggest.stale_review",
                    manual_fix_severity(policy),
                    format!("Manual fixer action needs review: {}", fix.description),
                    policy,
                )
            }
        };
        if let Some(subject) = fix.subject {
            suggestion = suggestion.with_subject(subject);
        }
        let command = match fix.fix_type {
            FixActionType::Auto => "cargo run -p wiki-cli -- fix --write --auto-only",
            FixActionType::Draft | FixActionType::Manual => {
                "cargo run -p wiki-cli -- fix --dry-run"
            }
        };
        builder.push(suggestion.with_suggested_command(command));
    }

    for finding in lint_findings {
        if finding.code != "claim.stale" && finding.code != "lifecycle.stale" {
            continue;
        }
        let mut suggestion = builder.suggestion(
            "suggest.supersede_candidate",
            StrategySeverity::High,
            finding.message,
            StrategyExecutionPolicy::AgentReview,
        );
        if let Some(subject) = finding.subject {
            let command =
                format!("cargo run -p wiki-cli -- supersede-claim {subject} \"<new claim text>\"");
            suggestion = suggestion
                .with_subject(subject)
                .with_suggested_command(command);
        }
        builder.push(suggestion);
    }

    for page in store.pages.values() {
        if !visible(&page.scope, viewer_scope) || page.status != EntryStatus::NeedsUpdate {
            continue;
        }
        builder.push(
            builder
                .suggestion(
                    "suggest.stale_review",
                    StrategySeverity::Medium,
                    "Page is marked NeedsUpdate and requires review",
                    StrategyExecutionPolicy::AgentReview,
                )
                .with_subject(page.id.0.to_string())
                .with_suggested_command(format!(
                    "cargo run -p wiki-cli -- promote-page {}",
                    page.id.0
                )),
        );
    }

    for group in repeated_visible_queries(store, viewer_scope, query_events) {
        builder.push(
            builder
                .suggestion(
                    "suggest.crystallize_candidate",
                    StrategySeverity::Medium,
                    format!(
                        "Visible query history repeated {} times for resolved top docs",
                        group.count
                    ),
                    StrategyExecutionPolicy::AgentReview,
                )
                .with_subject(format!("query_history:{}", group.top_doc_ids.join(",")))
                .with_suggested_command(format!(
                    "cargo run -p wiki-cli -- crystallize \"<redacted query>\" --finding \"top_docs={}\"",
                    group.top_doc_ids.join(",")
                )),
        );
    }

    let viewer_scope = viewer_scope.map(scope_label);
    let mut report = StrategyReport::new(
        report_id,
        Some(options.generated_at),
        viewer_scope,
        builder.suggestions,
    );
    if metrics.lifecycle.stale_claims > 0
        && !report
            .suggestions
            .iter()
            .any(|suggestion| suggestion.code == "suggest.supersede_candidate")
    {
        report.suggestions.push(StrategySuggestion::new(
            format!("{}-{:04}", report.report_id, report.suggestions.len() + 1),
            "suggest.stale_review",
            StrategySeverity::Medium,
            format!(
                "Metrics report shows {} stale claims",
                metrics.lifecycle.stale_claims
            ),
            StrategyExecutionPolicy::AgentReview,
        ));
    }
    report
}

struct StrategyReportBuilder {
    report_id: String,
    suggestions: Vec<StrategySuggestion>,
}

impl StrategyReportBuilder {
    fn new(report_id: String) -> Self {
        Self {
            report_id,
            suggestions: Vec::new(),
        }
    }

    fn suggestion(
        &self,
        code: impl Into<String>,
        severity: StrategySeverity,
        reason: impl Into<String>,
        execution_policy: StrategyExecutionPolicy,
    ) -> StrategySuggestion {
        StrategySuggestion::new(
            format!("{}-{:04}", self.report_id, self.suggestions.len() + 1),
            code,
            severity,
            reason,
            execution_policy,
        )
    }

    fn push(&mut self, suggestion: StrategySuggestion) {
        self.suggestions.push(suggestion);
    }
}

fn repeated_visible_queries(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
    events: &[WikiEvent],
) -> Vec<QueryGroup> {
    let mut counts: BTreeMap<String, QueryGroup> = BTreeMap::new();
    for event in events {
        let WikiEvent::QueryServed { top_doc_ids, .. } = event else {
            continue;
        };
        if query_doc_scope(store, viewer_scope, top_doc_ids).is_none() {
            continue;
        }
        let key = top_doc_ids.join("\n");
        let group = counts.entry(key).or_insert_with(|| QueryGroup {
            top_doc_ids: top_doc_ids.clone(),
            count: 0,
        });
        group.count += 1;
    }
    counts
        .into_values()
        .filter(|group| group.count >= 2)
        .collect()
}

struct QueryGroup {
    top_doc_ids: Vec<String>,
    count: usize,
}

fn query_doc_scope(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
    top_doc_ids: &[String],
) -> Option<Scope> {
    let mut scopes = Vec::new();
    for doc_id in top_doc_ids {
        scopes.push(doc_id_scope(store, viewer_scope, doc_id)?);
    }
    let first = scopes.first()?.clone();
    if scopes.iter().all(|scope| scope == &first) {
        Some(first)
    } else {
        None
    }
}

fn doc_id_scope(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
    doc_id: &str,
) -> Option<Scope> {
    if let Some(rest) = doc_id.strip_prefix("claim:") {
        let id = uuid::Uuid::parse_str(rest).ok()?;
        let claim = store.claims.get(&wiki_core::ClaimId(id))?;
        if claim.stale || !visible(&claim.scope, viewer_scope) {
            return None;
        }
        return Some(claim.scope.clone());
    }
    if let Some(rest) = doc_id.strip_prefix("page:") {
        let id = uuid::Uuid::parse_str(rest).ok()?;
        let page = store.pages.get(&wiki_core::PageId(id))?;
        if !visible(&page.scope, viewer_scope) {
            return None;
        }
        return Some(page.scope.clone());
    }
    if let Some(rest) = doc_id.strip_prefix("entity:") {
        let id = uuid::Uuid::parse_str(rest).ok()?;
        let entity = store.entities.get(&wiki_core::EntityId(id))?;
        if !visible(&entity.scope, viewer_scope) {
            return None;
        }
        return Some(entity.scope.clone());
    }
    if let Some(rest) = doc_id.strip_prefix("source:") {
        let id = uuid::Uuid::parse_str(rest).ok()?;
        let source = store.sources.get(&wiki_core::SourceId(id))?;
        if !visible(&source.scope, viewer_scope) {
            return None;
        }
        return Some(source.scope.clone());
    }
    None
}

fn visible(scope: &Scope, viewer_scope: Option<&Scope>) -> bool {
    viewer_scope
        .map(|viewer| document_visible_to_viewer(scope, viewer))
        .unwrap_or(true)
}

fn scope_label(scope: &Scope) -> String {
    match scope {
        Scope::Private { agent_id } => format!("private:{agent_id}"),
        Scope::Shared { team_id } => format!("shared:{team_id}"),
    }
}

fn manual_fix_execution_policy(code: &str) -> StrategyExecutionPolicy {
    match code {
        "page.broken_wikilink" | "gap.low_coverage" => StrategyExecutionPolicy::AgentReview,
        _ => StrategyExecutionPolicy::HumanRequired,
    }
}

fn manual_fix_severity(policy: StrategyExecutionPolicy) -> StrategySeverity {
    match policy {
        StrategyExecutionPolicy::HumanRequired => StrategySeverity::High,
        StrategyExecutionPolicy::AgentReview | StrategyExecutionPolicy::AutoSafe => {
            StrategySeverity::Medium
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{collect_wiki_metrics, format_claim_doc_id, format_page_doc_id, InMemoryStore};
    use time::OffsetDateTime;
    use uuid::Uuid;
    use wiki_core::{Claim, EntryType, MemoryTier, WikiPage};

    fn private_scope(agent_id: &str) -> Scope {
        Scope::Private {
            agent_id: agent_id.to_string(),
        }
    }

    fn generated_at() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    fn metrics(
        store: &InMemoryStore,
        schema: &DomainSchema,
        viewer_scope: Option<&Scope>,
    ) -> WikiMetricsReport {
        collect_wiki_metrics(store, schema, viewer_scope, None, None, 2, generated_at())
    }

    fn scan(
        store: &InMemoryStore,
        schema: &DomainSchema,
        viewer_scope: Option<&Scope>,
        query_events: &[WikiEvent],
    ) -> StrategyReport {
        let metrics = metrics(store, schema, viewer_scope);
        run_strategy_scan(
            store,
            schema,
            &metrics,
            query_events,
            StrategyScanOptions {
                viewer_scope,
                low_coverage_threshold: 2,
                generated_at: generated_at(),
                report_id: "report-1".to_string(),
            },
        )
    }

    #[test]
    fn auto_fix_generates_auto_safe_suggestion() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let page = WikiPage::new("Concept A", "## 定义\nOnly one section", scope.clone())
            .with_entry_type(EntryType::Concept);
        store.pages.insert(page.id, page);

        let report = scan(&store, &schema, Some(&scope), &[]);

        assert!(report.suggestions.iter().any(|suggestion| {
            suggestion.code == "suggest.fix_auto_safe"
                && suggestion.execution_policy == StrategyExecutionPolicy::AutoSafe
        }));
    }

    #[test]
    fn draft_fix_generates_agent_review_suggestion() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let page = WikiPage::new("Linked", "body", scope.clone());
        store.pages.insert(page.id, page);
        let claim = Claim::new("Linked", scope.clone(), MemoryTier::Semantic);
        let claim_id = claim.id.0.to_string();
        store.claims.insert(claim.id, claim);

        let report = scan(&store, &schema, Some(&scope), &[]);

        assert!(report.suggestions.iter().any(|suggestion| {
            suggestion.code == "suggest.stale_review"
                && suggestion.subject.as_deref() == Some(claim_id.as_str())
                && suggestion.execution_policy == StrategyExecutionPolicy::AgentReview
                && suggestion
                    .suggested_command
                    .as_deref()
                    .unwrap_or_default()
                    .contains("fix --dry-run")
        }));
    }

    #[test]
    fn manual_orphan_fix_requires_human() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let page = WikiPage::new("Orphan", "body", scope.clone());
        let page_id = page.id.0.to_string();
        store.pages.insert(page.id, page);

        let report = scan(&store, &schema, Some(&scope), &[]);

        assert!(report.suggestions.iter().any(|suggestion| {
            suggestion.code == "suggest.stale_review"
                && suggestion.subject.as_deref() == Some(page_id.as_str())
                && suggestion.execution_policy == StrategyExecutionPolicy::HumanRequired
                && suggestion.severity == StrategySeverity::High
        }));
    }

    #[test]
    fn manual_low_coverage_fix_can_go_to_agent_review() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let entity = wiki_core::Entity {
            id: wiki_core::EntityId(Uuid::new_v4()),
            kind: wiki_core::EntityKind::Concept,
            label: "Sparse".to_string(),
            scope: scope.clone(),
        };
        let entity_id = entity.id.0.to_string();
        store.entities.insert(entity.id, entity);

        let report = scan(&store, &schema, Some(&scope), &[]);

        assert!(report.suggestions.iter().any(|suggestion| {
            suggestion.code == "suggest.stale_review"
                && suggestion.subject.as_deref() == Some(entity_id.as_str())
                && suggestion.execution_policy == StrategyExecutionPolicy::AgentReview
        }));
    }

    #[test]
    fn stale_claim_generates_supersede_candidate() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let mut claim = Claim::new("old claim", scope.clone(), MemoryTier::Semantic);
        claim.stale = true;
        let claim_id = claim.id.0.to_string();
        store.claims.insert(claim.id, claim);

        let report = scan(&store, &schema, Some(&scope), &[]);

        assert!(report.suggestions.iter().any(|suggestion| {
            suggestion.code == "suggest.supersede_candidate"
                && suggestion.subject.as_deref() == Some(claim_id.as_str())
                && suggestion.execution_policy == StrategyExecutionPolicy::AgentReview
        }));
    }

    #[test]
    fn needs_update_page_generates_stale_review() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let page = WikiPage::new("Needs Update", "body", scope.clone())
            .with_status(EntryStatus::NeedsUpdate);
        let page_id = page.id.0.to_string();
        store.pages.insert(page.id, page);

        let report = scan(&store, &schema, Some(&scope), &[]);

        assert!(report.suggestions.iter().any(|suggestion| {
            suggestion.code == "suggest.stale_review"
                && suggestion.subject.as_deref() == Some(page_id.as_str())
        }));
    }

    #[test]
    fn visible_repeated_query_served_generates_crystallize_candidate() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let page = WikiPage::new("Visible", "body", scope.clone());
        let doc_id = format_page_doc_id(page.id);
        store.pages.insert(page.id, page);
        let events = vec![
            WikiEvent::QueryServed {
                query_fingerprint: "raw secret query".to_string(),
                top_doc_ids: vec![doc_id.clone()],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "raw secret query".to_string(),
                top_doc_ids: vec![doc_id],
                at: generated_at(),
            },
        ];

        let report = scan(&store, &schema, Some(&scope), &events);

        let suggestion = report
            .suggestions
            .iter()
            .find(|suggestion| suggestion.code == "suggest.crystallize_candidate")
            .expect("expected crystallize suggestion");
        assert_eq!(
            suggestion.execution_policy,
            StrategyExecutionPolicy::AgentReview
        );
        assert!(!suggestion.reason.contains("raw secret query"));
        assert!(!suggestion
            .suggested_command
            .as_deref()
            .unwrap_or_default()
            .contains("raw secret query"));
    }

    #[test]
    fn hidden_unresolved_mixed_scope_query_served_are_skipped() {
        let schema = DomainSchema::permissive_default();
        let viewer = private_scope("a");
        let hidden_scope = private_scope("b");
        let mut store = InMemoryStore::default();
        let visible_page = WikiPage::new("Visible", "body", viewer.clone());
        let hidden_page = WikiPage::new("Hidden", "body", hidden_scope);
        let visible_doc_id = format_page_doc_id(visible_page.id);
        let hidden_doc_id = format_page_doc_id(hidden_page.id);
        store.pages.insert(visible_page.id, visible_page);
        store.pages.insert(hidden_page.id, hidden_page);
        let unresolved_doc_id = format!("page:{}", Uuid::new_v4());
        let events = vec![
            WikiEvent::QueryServed {
                query_fingerprint: "hidden raw query".to_string(),
                top_doc_ids: vec![hidden_doc_id.clone()],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "hidden raw query".to_string(),
                top_doc_ids: vec![hidden_doc_id.clone()],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "unresolved raw query".to_string(),
                top_doc_ids: vec![unresolved_doc_id.clone()],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "unresolved raw query".to_string(),
                top_doc_ids: vec![unresolved_doc_id],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "mixed raw query".to_string(),
                top_doc_ids: vec![visible_doc_id.clone(), hidden_doc_id.clone()],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "empty raw query".to_string(),
                top_doc_ids: Vec::new(),
                at: generated_at(),
            },
        ];

        let report = scan(&store, &schema, Some(&viewer), &events);

        assert!(!report
            .suggestions
            .iter()
            .any(|suggestion| suggestion.code == "suggest.crystallize_candidate"));

        let mixed_events = vec![
            WikiEvent::QueryServed {
                query_fingerprint: "mixed raw query".to_string(),
                top_doc_ids: vec![visible_doc_id.clone(), hidden_doc_id.clone()],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "mixed raw query".to_string(),
                top_doc_ids: vec![visible_doc_id, hidden_doc_id],
                at: generated_at(),
            },
        ];
        let mixed_report = scan(&store, &schema, None, &mixed_events);
        assert!(!mixed_report
            .suggestions
            .iter()
            .any(|suggestion| suggestion.code == "suggest.crystallize_candidate"));
    }

    #[test]
    fn stale_query_claim_is_skipped() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let mut claim = Claim::new("old claim", scope.clone(), MemoryTier::Semantic);
        claim.stale = true;
        let doc_id = format_claim_doc_id(claim.id);
        store.claims.insert(claim.id, claim);
        let events = vec![
            WikiEvent::QueryServed {
                query_fingerprint: "stale raw query".to_string(),
                top_doc_ids: vec![doc_id.clone()],
                at: generated_at(),
            },
            WikiEvent::QueryServed {
                query_fingerprint: "stale raw query".to_string(),
                top_doc_ids: vec![doc_id],
                at: generated_at(),
            },
        ];

        let report = scan(&store, &schema, Some(&scope), &events);

        assert!(!report
            .suggestions
            .iter()
            .any(|suggestion| suggestion.code == "suggest.crystallize_candidate"));
    }

    #[test]
    fn scanner_does_not_change_store_or_query_events() {
        let schema = DomainSchema::permissive_default();
        let scope = private_scope("a");
        let mut store = InMemoryStore::default();
        let page = WikiPage::new("Concept A", "## 定义\nOnly one section", scope.clone())
            .with_entry_type(EntryType::Concept);
        store.pages.insert(page.id, page);
        let before = serde_json::to_value(store.to_snapshot(&[])).unwrap();
        let events = vec![WikiEvent::QueryServed {
            query_fingerprint: "raw query".to_string(),
            top_doc_ids: Vec::new(),
            at: generated_at(),
        }];
        let before_events = serde_json::to_value(&events).unwrap();

        let _report = scan(&store, &schema, Some(&scope), &events);

        let after = serde_json::to_value(store.to_snapshot(&[])).unwrap();
        let after_events = serde_json::to_value(&events).unwrap();
        assert_eq!(before, after);
        assert_eq!(before_events, after_events);
    }
}
