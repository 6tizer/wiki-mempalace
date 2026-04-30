use std::collections::{BTreeMap, BTreeSet, HashSet};

use time::OffsetDateTime;
use wiki_core::{
    document_visible_to_viewer, extract_headings, extract_wikilinks, Claim, DomainSchema,
    EntryType, GovernanceDuplicateGroup, GovernanceDuplicateMember, GovernanceGapFinding,
    GovernanceLifecycleSignal, GovernanceLintFinding, GovernanceReferenceFinding,
    GovernanceRetireCandidate, GovernanceScanReport, GovernanceSynthesisSignals,
    GovernanceTagIntersectionSignal, GovernanceTagSignal, MemoryTier, PromotionConditions,
    RawArtifact, Scope, WikiPage,
};

use crate::{collect_basic_lint_findings, run_gap_scan, InMemoryStore};

const MAX_NEAR_DUPLICATE_CLAIMS: usize = 2_000;

pub struct GovernanceScanOptions<'a> {
    pub viewer_scope: Option<&'a Scope>,
    pub low_coverage_threshold: usize,
    pub generated_at: OffsetDateTime,
    pub report_id: String,
}

pub fn run_governance_scan(
    store: &InMemoryStore,
    schema: &DomainSchema,
    options: GovernanceScanOptions<'_>,
) -> GovernanceScanReport {
    let viewer_scope = options.viewer_scope.map(scope_to_string);
    let mut report =
        GovernanceScanReport::new(options.report_id, options.generated_at, viewer_scope);

    let lint_findings = collect_basic_lint_findings(schema, store, options.viewer_scope);
    report.references = collect_reference_findings(store, &lint_findings, options.viewer_scope);
    report.lint = lint_findings
        .iter()
        .map(|finding| GovernanceLintFinding {
            code: finding.code.clone(),
            message: finding.message.clone(),
            severity: finding.severity,
            subject: finding.subject.clone(),
        })
        .collect();
    report.gaps = run_gap_scan(store, options.viewer_scope, options.low_coverage_threshold)
        .into_iter()
        .map(|finding| GovernanceGapFinding {
            code: finding.code,
            message: finding.message,
            severity: finding.severity,
            subject: finding.subject,
            subject_label: finding.subject_label,
        })
        .collect();
    report.lifecycle =
        collect_lifecycle_signals(store, schema, options.viewer_scope, options.generated_at);
    report.duplicates = collect_duplicate_groups(store, options.viewer_scope);
    report.retire_candidates = collect_retire_candidates(store, &report.lint, options.viewer_scope);
    report.synthesis_signals = collect_synthesis_signals(store, schema, options.viewer_scope);
    finalize_summary(&mut report);
    report
}

fn finalize_summary(report: &mut GovernanceScanReport) {
    report.summary.lifecycle_total = report.lifecycle.len() as u64;
    report.summary.lifecycle_promotable =
        report.lifecycle.iter().filter(|item| item.eligible).count() as u64;
    report.summary.lifecycle_blocked =
        report.summary.lifecycle_total - report.summary.lifecycle_promotable;
    report.summary.references_total = report.references.len() as u64;
    report.summary.lint_total = report.lint.len() as u64;
    report.summary.gaps_total = report.gaps.len() as u64;
    report.summary.duplicate_groups_total = report.duplicates.len() as u64;
    report.summary.retire_candidates_total = report.retire_candidates.len() as u64;
    report.summary.tag_signal_total = report.synthesis_signals.tags.len() as u64;
    report.summary.tag_intersection_total = report.synthesis_signals.intersections.len() as u64;
}

fn visible(scope: &Scope, viewer_scope: Option<&Scope>) -> bool {
    match viewer_scope {
        None => true,
        Some(viewer) => document_visible_to_viewer(scope, viewer),
    }
}

fn scope_to_string(scope: &Scope) -> String {
    match scope {
        Scope::Private { agent_id } => format!("private:{agent_id}"),
        Scope::Shared { team_id } => format!("shared:{team_id}"),
    }
}

fn tier_label(tier: MemoryTier) -> &'static str {
    match tier {
        MemoryTier::Working => "working",
        MemoryTier::Episodic => "episodic",
        MemoryTier::Semantic => "semantic",
        MemoryTier::Procedural => "procedural",
    }
}

fn collect_lifecycle_signals(
    store: &InMemoryStore,
    schema: &DomainSchema,
    viewer_scope: Option<&Scope>,
    now: OffsetDateTime,
) -> Vec<GovernanceLifecycleSignal> {
    let mut out = Vec::new();
    for page in sorted_visible_pages(store, viewer_scope) {
        if let Some(entry_type) = page.entry_type.as_ref() {
            if let Some(rule) = schema.find_lifecycle_rule(entry_type) {
                let next = rule
                    .promotions
                    .iter()
                    .find(|promotion| promotion.from_status == page.status);
                let (next_status, blockers) = match next {
                    Some(promotion) => (
                        Some(promotion.to_status),
                        page_promotion_blockers(
                            store,
                            page,
                            &promotion.conditions,
                            viewer_scope,
                            now,
                        ),
                    ),
                    None => (None, vec!["no_next_promotion".to_string()]),
                };
                out.push(GovernanceLifecycleSignal {
                    subject_type: "page".into(),
                    subject_id: page.id.0.to_string(),
                    label: Some(page.title.clone()),
                    entry_type: Some(entry_type.clone()),
                    status: Some(page.status),
                    tier: None,
                    next_status,
                    eligible: next_status.is_some() && blockers.is_empty(),
                    blockers,
                });
            }
        }
    }

    for claim in sorted_visible_claims(store, viewer_scope) {
        let mut blockers = Vec::new();
        if claim.stale {
            blockers.push("claim_stale".to_string());
        }
        if claim.confidence < schema.min_confidence_to_promote {
            blockers.push(format!(
                "confidence_below_threshold:{:.3}<{:.3}",
                claim.confidence, schema.min_confidence_to_promote
            ));
        }
        if claim.quality_score < schema.min_quality_to_crystallize {
            blockers.push(format!(
                "quality_below_threshold:{:.3}<{:.3}",
                claim.quality_score, schema.min_quality_to_crystallize
            ));
        }
        out.push(GovernanceLifecycleSignal {
            subject_type: "claim".into(),
            subject_id: claim.id.0.to_string(),
            label: Some(truncate_chars(&claim.text, 80)),
            entry_type: None,
            status: None,
            tier: Some(tier_label(claim.tier).to_string()),
            next_status: None,
            eligible: blockers.is_empty(),
            blockers,
        });
    }

    out
}

fn page_promotion_blockers(
    store: &InMemoryStore,
    page: &WikiPage,
    conditions: &PromotionConditions,
    viewer_scope: Option<&Scope>,
    now: OffsetDateTime,
) -> Vec<String> {
    let mut blockers = Vec::new();
    let age_days = ((now - page.age_from()).whole_seconds().max(0) as u64) / 86_400;
    if age_days < conditions.min_age_days {
        blockers.push(format!("age_days:{age_days}<{}", conditions.min_age_days));
    }

    let headings: HashSet<_> = extract_headings(&page.markdown).into_iter().collect();
    let missing_sections: Vec<_> = conditions
        .required_sections
        .iter()
        .filter(|section| !headings.contains(*section))
        .cloned()
        .collect();
    if !missing_sections.is_empty() {
        blockers.push(format!("missing_sections:{}", missing_sections.join(",")));
    }

    let inbound = store
        .pages
        .values()
        .filter(|candidate| visible(&candidate.scope, viewer_scope))
        .filter(|candidate| candidate.id != page.id)
        .flat_map(|candidate| extract_wikilinks(&candidate.markdown))
        .filter(|link| link == &page.title)
        .count() as u32;
    if inbound < conditions.min_references {
        blockers.push(format!(
            "references:{inbound}<{}",
            conditions.min_references
        ));
    }

    if let Some(cooldown_days) = conditions.cooldown_days {
        let actual = ((now - page.status_since()).whole_seconds().max(0) as u64) / 86_400;
        if actual < cooldown_days {
            blockers.push(format!("cooldown_days:{actual}<{cooldown_days}"));
        }
    }

    blockers
}

fn collect_reference_findings(
    store: &InMemoryStore,
    lint: &[wiki_core::LintFinding],
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceReferenceFinding> {
    let mut out = Vec::new();

    for claim in sorted_visible_claims(store, viewer_scope) {
        if claim.source_ids.is_empty() {
            out.push(reference_finding(
                "reference.missing_source",
                "claim has no source_ids",
                "warn",
                "claim",
                Some(claim.id.0.to_string()),
                Some(truncate_chars(&claim.text, 80)),
            ));
        }
        let mut seen = HashSet::new();
        for source_id in &claim.source_ids {
            if !seen.insert(source_id.0) {
                out.push(reference_finding(
                    "reference.duplicate_source_id",
                    "claim repeats the same source_id",
                    "info",
                    "claim",
                    Some(claim.id.0.to_string()),
                    Some(truncate_chars(&claim.text, 80)),
                ));
            }
            match store.sources.get(source_id) {
                Some(source) if visible(&source.scope, viewer_scope) => {}
                _ => out.push(reference_finding(
                    "reference.broken_source_id",
                    "claim points to a missing or invisible source_id",
                    "warn",
                    "claim",
                    Some(claim.id.0.to_string()),
                    Some(source_id.0.to_string()),
                )),
            }
        }
    }

    for page in sorted_visible_pages(store, viewer_scope) {
        if page_requires_source(page)
            && page
                .source_url
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && urls_in_text(&page.markdown).is_empty()
        {
            out.push(reference_finding(
                "reference.missing_source",
                "page has reference section but no structured source_url or URL evidence",
                "warn",
                "page",
                Some(page.id.0.to_string()),
                Some(page.title.clone()),
            ));
        }

        let urls = urls_in_text(&page.markdown);
        if !urls.is_empty() {
            out.push(reference_finding(
                "reference.raw_url",
                "page contains raw URL text; prefer structured source metadata",
                "info",
                "page",
                Some(page.id.0.to_string()),
                Some(page.title.clone()),
            ));
            let mut seen = HashSet::new();
            for url in urls {
                if !seen.insert(url.clone()) {
                    out.push(reference_finding(
                        "reference.duplicate_raw_url",
                        "page repeats the same raw URL",
                        "info",
                        "page",
                        Some(page.id.0.to_string()),
                        Some(url),
                    ));
                }
            }
        }
    }

    for finding in lint
        .iter()
        .filter(|finding| finding.code == "page.broken_wikilink")
    {
        out.push(reference_finding(
            "reference.broken_wikilink",
            &finding.message,
            "warn",
            "page",
            finding.subject.clone(),
            None,
        ));
    }

    out
}

fn reference_finding(
    code: &str,
    message: &str,
    severity: &str,
    subject_type: &str,
    subject_id: Option<String>,
    label: Option<String>,
) -> GovernanceReferenceFinding {
    GovernanceReferenceFinding {
        code: code.into(),
        message: message.into(),
        severity: severity.into(),
        subject_type: subject_type.into(),
        subject_id,
        label,
    }
}

fn page_requires_source(page: &WikiPage) -> bool {
    matches!(
        page.entry_type,
        Some(EntryType::Concept | EntryType::Entity | EntryType::Synthesis)
    ) && (page.markdown.contains("来源引用") || page.markdown.contains("来源列表"))
}

fn collect_duplicate_groups(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceDuplicateGroup> {
    let mut groups = Vec::new();
    groups.extend(duplicate_sources_by_uri(store, viewer_scope));
    groups.extend(duplicate_pages_by_body(store, viewer_scope));
    groups.extend(duplicate_claims_by_text(store, viewer_scope));
    groups.extend(near_duplicate_pages_by_title(store, viewer_scope));
    groups.extend(near_duplicate_claims_by_tokens(store, viewer_scope));
    groups
}

fn duplicate_sources_by_uri(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceDuplicateGroup> {
    let mut by_uri: BTreeMap<String, Vec<&RawArtifact>> = BTreeMap::new();
    for source in sorted_visible_sources(store, viewer_scope) {
        let key = source.uri.trim().to_ascii_lowercase();
        if !key.is_empty() {
            by_uri.entry(key).or_default().push(source);
        }
    }
    by_uri
        .into_iter()
        .filter(|(_, items)| items.len() > 1)
        .map(|(key, items)| duplicate_group("source_url", key, "exact", items, source_member))
        .collect()
}

fn duplicate_pages_by_body(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceDuplicateGroup> {
    let mut by_body: BTreeMap<String, Vec<&WikiPage>> = BTreeMap::new();
    for page in sorted_visible_pages(store, viewer_scope) {
        let key = normalize_text(&page.markdown);
        if key.chars().count() >= 32 {
            by_body.entry(key).or_default().push(page);
        }
    }
    by_body
        .into_iter()
        .filter(|(_, items)| items.len() > 1)
        .map(|(key, items)| duplicate_group("page_body", key, "exact", items, page_member))
        .collect()
}

fn duplicate_claims_by_text(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceDuplicateGroup> {
    let mut by_text: BTreeMap<String, Vec<&Claim>> = BTreeMap::new();
    for claim in sorted_visible_claims(store, viewer_scope) {
        let key = normalize_text(&claim.text);
        if key.chars().count() >= 16 {
            by_text.entry(key).or_default().push(claim);
        }
    }
    by_text
        .into_iter()
        .filter(|(_, items)| items.len() > 1)
        .map(|(key, items)| duplicate_group("claim_text", key, "exact", items, claim_member))
        .collect()
}

fn duplicate_group<T>(
    kind: &str,
    key: String,
    confidence: &str,
    items: Vec<&T>,
    member: fn(&T) -> GovernanceDuplicateMember,
) -> GovernanceDuplicateGroup {
    GovernanceDuplicateGroup {
        kind: kind.into(),
        key: truncate_chars(&key, 160),
        confidence: confidence.into(),
        members: items.into_iter().map(member).collect(),
    }
}

fn near_duplicate_pages_by_title(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceDuplicateGroup> {
    let mut by_title: BTreeMap<String, Vec<&WikiPage>> = BTreeMap::new();
    for page in sorted_visible_pages(store, viewer_scope) {
        let key = normalize_text(&page.title);
        if key.chars().count() >= 4 {
            by_title.entry(key).or_default().push(page);
        }
    }
    by_title
        .into_iter()
        .filter(|(_, items)| items.len() > 1)
        .map(|(key, items)| duplicate_group("page_title", key, "near", items, page_member))
        .collect()
}

fn near_duplicate_claims_by_tokens(
    store: &InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceDuplicateGroup> {
    let claims = sorted_visible_claims(store, viewer_scope);
    let indexed: Vec<_> = claims
        .into_iter()
        .take(MAX_NEAR_DUPLICATE_CLAIMS)
        .map(|claim| (claim, token_set(&claim.text)))
        .collect();
    let mut groups = Vec::new();
    let mut used_pairs = HashSet::new();
    for i in 0..indexed.len() {
        let claim_a = indexed[i].0;
        let a_tokens = &indexed[i].1;
        if a_tokens.len() < 5 {
            continue;
        }
        for (claim_b, b_tokens) in indexed.iter().skip(i + 1) {
            if b_tokens.len() < 5 {
                continue;
            }
            if jaccard(a_tokens, b_tokens) < 0.85 {
                continue;
            }
            let pair = (claim_a.id.0, claim_b.id.0);
            if used_pairs.insert(pair) {
                groups.push(GovernanceDuplicateGroup {
                    kind: "claim_text".into(),
                    key: truncate_chars(&claim_a.text, 160),
                    confidence: "near".into(),
                    members: vec![claim_member(claim_a), claim_member(claim_b)],
                });
            }
            if groups.len() >= 100 {
                return groups;
            }
        }
    }
    groups
}

fn collect_retire_candidates(
    store: &InMemoryStore,
    lint: &[GovernanceLintFinding],
    viewer_scope: Option<&Scope>,
) -> Vec<GovernanceRetireCandidate> {
    let orphan_page_ids: HashSet<String> = lint
        .iter()
        .filter(|finding| finding.code == "page.orphan")
        .filter_map(|finding| finding.subject.clone())
        .collect();
    let mut out = Vec::new();
    for page in sorted_visible_pages(store, viewer_scope) {
        let mut evidence = Vec::new();
        let reason = if page.markdown.trim().is_empty() {
            evidence.push("markdown_empty".to_string());
            Some("empty_page")
        } else if orphan_page_ids.contains(&page.id.0.to_string())
            && page.compiled_by.is_some()
            && page.source_url.is_none()
        {
            evidence.push("lint:page.orphan".to_string());
            evidence.push("compiled_by_present".to_string());
            evidence.push("source_url_missing".to_string());
            Some("orphan_projection_page")
        } else if normalize_text(&page.title).contains("merged")
            || page.title.contains("已合并")
            || page.title.contains("合并残留")
        {
            evidence.push("title_marks_merged_residue".to_string());
            Some("merged_residue")
        } else {
            None
        };

        if let Some(reason) = reason {
            out.push(GovernanceRetireCandidate {
                page_id: page.id.0.to_string(),
                title: page.title.clone(),
                reason: reason.into(),
                evidence,
            });
        }
    }
    out
}

fn collect_synthesis_signals(
    store: &InMemoryStore,
    schema: &DomainSchema,
    viewer_scope: Option<&Scope>,
) -> GovernanceSynthesisSignals {
    let deprecated: HashSet<String> = schema
        .tag_config
        .deprecated_tags
        .iter()
        .map(|tag| tag.to_ascii_lowercase())
        .collect();
    let mut tag_pages: BTreeMap<String, Vec<&WikiPage>> = BTreeMap::new();
    let mut deprecated_used = BTreeSet::new();

    for page in sorted_visible_pages(store, viewer_scope) {
        for tag in &page.tags {
            let key = tag.to_ascii_lowercase();
            if deprecated.contains(&key) {
                deprecated_used.insert(tag.clone());
                continue;
            }
            if matches!(
                page.entry_type,
                Some(EntryType::Concept | EntryType::Entity)
            ) {
                tag_pages.entry(tag.clone()).or_default().push(page);
            }
        }
    }
    for claim in sorted_visible_claims(store, viewer_scope) {
        for tag in &claim.tags {
            if deprecated.contains(&tag.to_ascii_lowercase()) {
                deprecated_used.insert(tag.clone());
            }
        }
    }
    for source in sorted_visible_sources(store, viewer_scope) {
        for tag in &source.tags {
            if deprecated.contains(&tag.to_ascii_lowercase()) {
                deprecated_used.insert(tag.clone());
            }
        }
    }

    let mut tags = Vec::new();
    for (tag, pages) in &tag_pages {
        let mut page_ids: Vec<_> = pages.iter().map(|page| page.id.0.to_string()).collect();
        page_ids.sort();
        let mut domains: Vec<_> = pages
            .iter()
            .filter_map(|page| page.source_url.as_deref())
            .filter_map(domain_from_url)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        domains.sort();
        tags.push(GovernanceTagSignal {
            tag: tag.clone(),
            concept_entity_pages: pages.len() as u64,
            page_ids,
            source_domains: domains,
        });
    }
    tags.sort_by(|a, b| {
        b.concept_entity_pages
            .cmp(&a.concept_entity_pages)
            .then_with(|| a.tag.cmp(&b.tag))
    });

    let tag_names: Vec<_> = tag_pages.keys().cloned().collect();
    let mut intersections = Vec::new();
    for i in 0..tag_names.len() {
        let a = &tag_names[i];
        let a_ids: HashSet<_> = tag_pages[a].iter().map(|page| page.id.0).collect();
        for b in tag_names.iter().skip(i + 1) {
            let mut page_ids: Vec<_> = tag_pages[b]
                .iter()
                .filter(|page| a_ids.contains(&page.id.0))
                .map(|page| page.id.0.to_string())
                .collect();
            if page_ids.len() >= 2 {
                page_ids.sort();
                intersections.push(GovernanceTagIntersectionSignal {
                    tags: vec![a.clone(), b.clone()],
                    concept_entity_pages: page_ids.len() as u64,
                    page_ids,
                });
            }
        }
    }
    intersections.sort_by(|a, b| {
        b.concept_entity_pages
            .cmp(&a.concept_entity_pages)
            .then_with(|| a.tags.cmp(&b.tags))
    });

    GovernanceSynthesisSignals {
        tags,
        intersections,
        deprecated_tags_used: deprecated_used.into_iter().collect(),
    }
}

fn source_member(source: &RawArtifact) -> GovernanceDuplicateMember {
    GovernanceDuplicateMember {
        subject_type: "source".into(),
        subject_id: source.id.0.to_string(),
        label: Some(source.uri.clone()),
    }
}

fn page_member(page: &WikiPage) -> GovernanceDuplicateMember {
    GovernanceDuplicateMember {
        subject_type: "page".into(),
        subject_id: page.id.0.to_string(),
        label: Some(page.title.clone()),
    }
}

fn claim_member(claim: &Claim) -> GovernanceDuplicateMember {
    GovernanceDuplicateMember {
        subject_type: "claim".into(),
        subject_id: claim.id.0.to_string(),
        label: Some(truncate_chars(&claim.text, 80)),
    }
}

fn sorted_visible_sources<'a>(
    store: &'a InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<&'a RawArtifact> {
    let mut items: Vec<_> = store
        .sources
        .values()
        .filter(|source| visible(&source.scope, viewer_scope))
        .collect();
    items.sort_by_key(|source| source.id.0);
    items
}

fn sorted_visible_pages<'a>(
    store: &'a InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<&'a WikiPage> {
    let mut items: Vec<_> = store
        .pages
        .values()
        .filter(|page| visible(&page.scope, viewer_scope))
        .collect();
    items.sort_by_key(|page| page.id.0);
    items
}

fn sorted_visible_claims<'a>(
    store: &'a InMemoryStore,
    viewer_scope: Option<&Scope>,
) -> Vec<&'a Claim> {
    let mut items: Vec<_> = store
        .claims
        .values()
        .filter(|claim| visible(&claim.scope, viewer_scope))
        .collect();
    items.sort_by_key(|claim| claim.id.0);
    items
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_ascii_lowercase()
}

fn token_set(value: &str) -> HashSet<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .map(str::trim)
        .filter(|part| part.chars().count() >= 2)
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn urls_in_text(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for part in text.split_whitespace() {
        let trimmed = part.trim_matches(|ch: char| {
            matches!(
                ch,
                '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | ',' | ';'
            )
        });
        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            urls.push(trimmed.trim_end_matches(['.', ',', ')', ']']).to_string());
        }
    }
    urls
}

fn domain_from_url(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    without_scheme
        .split('/')
        .next()
        .map(str::trim)
        .filter(|domain| !domain.is_empty())
        .map(|domain| domain.trim_start_matches("www.").to_ascii_lowercase())
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= limit {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{
        Claim, EntryStatus, LifecycleRule, MemoryTier, PromotionConditions, PromotionRule,
    };

    fn private(agent_id: &str) -> Scope {
        Scope::Private {
            agent_id: agent_id.into(),
        }
    }

    #[test]
    fn scan_filters_scope_and_detects_core_findings() {
        let viewer = private("a");
        let other = private("b");
        let mut store = InMemoryStore::default();
        let source_a = RawArtifact::new("https://example.com/a", "body", viewer.clone());
        let source_b = RawArtifact::new("https://example.com/a", "other", viewer.clone());
        let hidden_source = RawArtifact::new("https://example.com/a", "hidden", other.clone());
        store.sources.insert(source_a.id, source_a.clone());
        store.sources.insert(source_b.id, source_b.clone());
        store.sources.insert(hidden_source.id, hidden_source);

        let mut claim = Claim::new(
            "alpha beta gamma delta epsilon zeta",
            viewer.clone(),
            MemoryTier::Semantic,
        );
        claim.source_ids = vec![source_a.id, source_a.id];
        store.claims.insert(claim.id, claim);
        let hidden_claim = Claim::new("hidden claim", other, MemoryTier::Semantic);
        store.claims.insert(hidden_claim.id, hidden_claim);

        let page = WikiPage::new("Alpha", "", viewer.clone()).with_entry_type(EntryType::Concept);
        store.pages.insert(page.id, page);

        let report = run_governance_scan(
            &store,
            &DomainSchema::permissive_default(),
            GovernanceScanOptions {
                viewer_scope: Some(&viewer),
                low_coverage_threshold: 2,
                generated_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
                report_id: "test".into(),
            },
        );

        assert_eq!(report.viewer_scope.as_deref(), Some("private:a"));
        assert!(report
            .duplicates
            .iter()
            .any(|group| group.kind == "source_url" && group.members.len() == 2));
        assert!(report
            .references
            .iter()
            .any(|finding| finding.code == "reference.duplicate_source_id"));
        assert_eq!(report.retire_candidates.len(), 1);
        assert_eq!(report.retire_candidates[0].reason, "empty_page");
        assert!(report.summary.lint_total > 0);
    }

    #[test]
    fn lifecycle_reports_page_promotion_blockers() {
        let viewer = private("a");
        let mut schema = DomainSchema::permissive_default();
        schema.lifecycle_rules = vec![LifecycleRule {
            entry_types: vec![EntryType::Concept],
            initial_status: EntryStatus::Draft,
            promotions: vec![PromotionRule {
                from_status: EntryStatus::Draft,
                to_status: EntryStatus::InReview,
                conditions: PromotionConditions {
                    min_age_days: 0,
                    required_sections: vec!["来源引用".into()],
                    min_references: 1,
                    cooldown_days: None,
                },
            }],
            stale_days: None,
            auto_cleanup: false,
        }];

        let mut store = InMemoryStore::default();
        let page = WikiPage::new("Alpha", "## 定义\nbody", viewer.clone())
            .with_entry_type(EntryType::Concept);
        store.pages.insert(page.id, page);
        let hidden_ref = WikiPage::new(
            "Hidden Ref",
            "This hidden page links to [[Alpha]].",
            private("b"),
        );
        store.pages.insert(hidden_ref.id, hidden_ref);

        let report = run_governance_scan(
            &store,
            &schema,
            GovernanceScanOptions {
                viewer_scope: Some(&viewer),
                low_coverage_threshold: 2,
                generated_at: OffsetDateTime::now_utc(),
                report_id: "test".into(),
            },
        );

        let page_signal = report
            .lifecycle
            .iter()
            .find(|signal| signal.subject_type == "page")
            .expect("page lifecycle signal");
        assert_eq!(page_signal.next_status, Some(EntryStatus::InReview));
        assert!(!page_signal.eligible);
        assert!(page_signal
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("missing_sections:")));
        assert!(page_signal
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("references:")));
    }

    #[test]
    fn synthesis_signals_ignore_deprecated_tags_and_build_intersections() {
        let viewer = private("a");
        let mut schema = DomainSchema::permissive_default();
        schema.tag_config.deprecated_tags = vec!["old".into()];
        let mut store = InMemoryStore::default();
        for idx in 0..3 {
            let mut page = WikiPage::new(format!("P{idx}"), "body", viewer.clone())
                .with_entry_type(EntryType::Concept);
            page.tags = vec!["AI".into(), "Memory".into(), "old".into()];
            page.source_url = Some(format!("https://example{idx}.com/a"));
            store.pages.insert(page.id, page);
        }

        let report = run_governance_scan(
            &store,
            &schema,
            GovernanceScanOptions {
                viewer_scope: Some(&viewer),
                low_coverage_threshold: 2,
                generated_at: OffsetDateTime::now_utc(),
                report_id: "test".into(),
            },
        );

        assert!(report
            .synthesis_signals
            .tags
            .iter()
            .any(|tag| tag.tag == "AI" && tag.concept_entity_pages == 3));
        assert!(report
            .synthesis_signals
            .intersections
            .iter()
            .any(|intersection| intersection.tags == vec!["AI", "Memory"]));
        assert_eq!(report.synthesis_signals.deprecated_tags_used, vec!["old"]);
    }
}
