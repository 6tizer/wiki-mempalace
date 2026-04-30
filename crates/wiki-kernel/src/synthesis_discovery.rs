use std::collections::{BTreeMap, BTreeSet};

use time::OffsetDateTime;
use wiki_core::{
    GovernanceScanReport, GovernanceTagIntersectionSignal, GovernanceTagSignal, SynthesisCandidate,
    SynthesisCandidateKind, SynthesisDiscoveryReport,
};

pub struct SynthesisDiscoveryOptions {
    pub generated_at: OffsetDateTime,
    pub report_id: String,
    pub max_single_double: usize,
    pub max_triple: usize,
    pub max_quad: usize,
}

pub fn discover_synthesis_candidates(
    scan: &GovernanceScanReport,
    options: SynthesisDiscoveryOptions,
) -> SynthesisDiscoveryReport {
    let tag_map: BTreeMap<String, GovernanceTagSignal> = scan
        .synthesis_signals
        .tags
        .iter()
        .map(|tag| (tag.tag.clone(), tag.clone()))
        .collect();
    let pair_map = pair_intersection_map(&scan.synthesis_signals.intersections);
    let covered_tag_sets: BTreeSet<Vec<String>> = scan
        .synthesis_signals
        .existing_topics
        .iter()
        .map(|topic| normalized_tags(&topic.tags))
        .filter(|tags| !tags.is_empty())
        .collect();
    let existing_triples: BTreeSet<Vec<String>> = scan
        .synthesis_signals
        .existing_topics
        .iter()
        .map(|topic| normalized_tags(&topic.tags))
        .filter(|tags| tags.len() == 3)
        .collect();
    let existing_doubles: BTreeSet<Vec<String>> = scan
        .synthesis_signals
        .existing_topics
        .iter()
        .map(|topic| normalized_tags(&topic.tags))
        .filter(|tags| tags.len() == 2)
        .collect();

    let mut single_pool = single_tag_candidates(&tag_map, &covered_tag_sets);
    let mut double_pool = double_tag_candidates(&tag_map, &pair_map, &covered_tag_sets);
    let mut triple_pool = triple_tag_candidates(&tag_map, &pair_map, &covered_tag_sets);
    let mut quad_pool = quad_tag_candidates(
        &tag_map,
        &pair_map,
        &covered_tag_sets,
        &existing_triples,
        &existing_doubles,
        &triple_pool,
    );

    sort_candidates(&mut single_pool);
    sort_candidates(&mut double_pool);
    sort_candidates(&mut triple_pool);
    sort_candidates(&mut quad_pool);

    let mut single_double_pool = Vec::new();
    single_double_pool.extend(single_pool);
    single_double_pool.extend(double_pool);
    sort_candidates(&mut single_double_pool);

    let mut candidates = Vec::new();
    candidates.extend(quad_pool.into_iter().take(options.max_quad));
    candidates.extend(triple_pool.into_iter().take(options.max_triple));
    candidates.extend(
        single_double_pool
            .into_iter()
            .take(options.max_single_double),
    );
    sort_candidates(&mut candidates);
    for (idx, candidate) in candidates.iter_mut().enumerate() {
        candidate.candidate_id = format!("synth-cand-{:04}", idx + 1);
    }

    let mut report = SynthesisDiscoveryReport::new(
        options.report_id,
        options.generated_at,
        scan.viewer_scope.clone(),
        scan.report_id.clone(),
    );
    report.candidates = candidates;
    report.refresh_summary();
    report
}

fn single_tag_candidates(
    tag_map: &BTreeMap<String, GovernanceTagSignal>,
    covered_tag_sets: &BTreeSet<Vec<String>>,
) -> Vec<SynthesisCandidate> {
    tag_map
        .values()
        .filter(|tag| tag.concept_entity_pages >= 10)
        .filter_map(|tag| {
            let tags = vec![tag.tag.clone()];
            if covered_tag_sets.contains(&tags) {
                return None;
            }
            Some(candidate(
                SynthesisCandidateKind::SingleTag,
                tags,
                Vec::new(),
                tag.page_ids.clone(),
                tag.source_domains.clone(),
                "single tag has at least 10 concept/entity pages",
            ))
        })
        .collect()
}

fn double_tag_candidates(
    tag_map: &BTreeMap<String, GovernanceTagSignal>,
    pair_map: &BTreeMap<Vec<String>, GovernanceTagIntersectionSignal>,
    covered_tag_sets: &BTreeSet<Vec<String>>,
) -> Vec<SynthesisCandidate> {
    pair_map
        .values()
        .filter(|pair| pair.concept_entity_pages >= 3)
        .filter_map(|pair| {
            let tags = normalized_tags(&pair.tags);
            if covered_tag_sets.contains(&tags) {
                return None;
            }
            Some(candidate(
                SynthesisCandidateKind::DoubleTag,
                tags.clone(),
                Vec::new(),
                pair.page_ids.clone(),
                source_domains_for_tags(tag_map, &tags),
                "two tags share at least 3 concept/entity pages",
            ))
        })
        .collect()
}

fn triple_tag_candidates(
    tag_map: &BTreeMap<String, GovernanceTagSignal>,
    pair_map: &BTreeMap<Vec<String>, GovernanceTagIntersectionSignal>,
    covered_tag_sets: &BTreeSet<Vec<String>>,
) -> Vec<SynthesisCandidate> {
    let eligible_tags: BTreeSet<_> = tag_map
        .values()
        .filter(|tag| tag.concept_entity_pages >= 5)
        .map(|tag| tag.tag.clone())
        .collect();
    let pair_graph = pair_graph(pair_map, &eligible_tags);
    let mut out = Vec::new();
    for (a, a_neighbors) in &pair_graph {
        for b in a_neighbors.iter().filter(|b| *b > a) {
            let Some(b_neighbors) = pair_graph.get(b) else {
                continue;
            };
            for c in a_neighbors.intersection(b_neighbors).filter(|c| *c > b) {
                let tags = vec![a.clone(), b.clone(), c.clone()];
                if covered_tag_sets.contains(&tags) {
                    continue;
                }
                out.push(candidate(
                    SynthesisCandidateKind::TripleTag,
                    tags.clone(),
                    Vec::new(),
                    pair_page_union(pair_map, &tags),
                    source_domains_for_tags(tag_map, &tags),
                    "three tags each have at least 5 pages and all three pairwise edges have at least 2 shared pages",
                ));
            }
        }
    }
    out
}

fn pair_graph(
    pair_map: &BTreeMap<Vec<String>, GovernanceTagIntersectionSignal>,
    eligible_tags: &BTreeSet<String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for pair in pair_map
        .values()
        .filter(|pair| pair.concept_entity_pages >= 2)
    {
        let tags = normalized_tags(&pair.tags);
        if tags.len() != 2 || !eligible_tags.contains(&tags[0]) || !eligible_tags.contains(&tags[1])
        {
            continue;
        }
        graph
            .entry(tags[0].clone())
            .or_default()
            .insert(tags[1].clone());
        graph
            .entry(tags[1].clone())
            .or_default()
            .insert(tags[0].clone());
    }
    graph
}

fn quad_tag_candidates(
    tag_map: &BTreeMap<String, GovernanceTagSignal>,
    pair_map: &BTreeMap<Vec<String>, GovernanceTagIntersectionSignal>,
    covered_tag_sets: &BTreeSet<Vec<String>>,
    existing_triples: &BTreeSet<Vec<String>>,
    existing_doubles: &BTreeSet<Vec<String>>,
    triple_pool: &[SynthesisCandidate],
) -> Vec<SynthesisCandidate> {
    let eligible_d_tags: Vec<_> = tag_map
        .values()
        .filter(|tag| tag.concept_entity_pages >= 5)
        .map(|tag| tag.tag.clone())
        .collect();
    let mut out = Vec::new();
    for triple in triple_pool {
        let anchor = normalized_tags(&triple.tags);
        if !has_quad_anchor(&anchor, existing_triples, existing_doubles) {
            continue;
        }
        for d in &eligible_d_tags {
            if anchor.contains(d) {
                continue;
            }
            if anchor.iter().any(|tag| pair_count(pair_map, tag, d) > 3) {
                continue;
            }
            let mut tags = anchor.clone();
            tags.push(d.clone());
            tags.sort();
            if covered_tag_sets.contains(&tags) {
                continue;
            }
            let mut page_ids = page_union_for_tags(tag_map, &tags);
            page_ids.extend(triple.page_ids.clone());
            page_ids = dedupe_sorted(page_ids);
            out.push(candidate(
                SynthesisCandidateKind::QuadTag,
                tags,
                vec![anchor.clone()],
                page_ids,
                source_domains_for_tags(tag_map, &anchor_with_d(&anchor, d)),
                "existing synthesis anchors a three-tag triangle and the fourth tag is distant from every anchor tag",
            ));
        }
    }
    out
}

fn has_quad_anchor(
    triple_tags: &[String],
    existing_triples: &BTreeSet<Vec<String>>,
    existing_doubles: &BTreeSet<Vec<String>>,
) -> bool {
    if triple_tags.len() != 3 {
        return false;
    }
    if existing_triples.contains(&triple_tags.to_vec()) {
        return true;
    }
    let pairs = [
        normalized_tags(&[triple_tags[0].clone(), triple_tags[1].clone()]),
        normalized_tags(&[triple_tags[0].clone(), triple_tags[2].clone()]),
        normalized_tags(&[triple_tags[1].clone(), triple_tags[2].clone()]),
    ];
    pairs
        .iter()
        .filter(|pair| existing_doubles.contains(*pair))
        .count()
        >= 2
}

fn candidate(
    kind: SynthesisCandidateKind,
    tags: Vec<String>,
    anchor_tags: Vec<Vec<String>>,
    page_ids: Vec<String>,
    source_domains: Vec<String>,
    rationale: &str,
) -> SynthesisCandidate {
    let page_ids = dedupe_sorted(page_ids);
    let source_domains = dedupe_sorted(source_domains);
    let score =
        kind_rank(kind) * 1_000_000 + source_domains.len() as i64 * 10_000 + page_ids.len() as i64;
    SynthesisCandidate {
        candidate_id: String::new(),
        kind,
        tags,
        anchor_tags,
        concept_entity_pages: page_ids.len() as u64,
        page_ids,
        source_domains,
        score,
        rationale: rationale.to_string(),
    }
}

fn kind_rank(kind: SynthesisCandidateKind) -> i64 {
    match kind {
        SynthesisCandidateKind::SingleTag => 1,
        SynthesisCandidateKind::DoubleTag => 2,
        SynthesisCandidateKind::TripleTag => 3,
        SynthesisCandidateKind::QuadTag => 4,
    }
}

fn sort_candidates(candidates: &mut [SynthesisCandidate]) {
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.tags.cmp(&b.tags))
            .then_with(|| a.page_ids.cmp(&b.page_ids))
    });
}

fn pair_intersection_map(
    intersections: &[GovernanceTagIntersectionSignal],
) -> BTreeMap<Vec<String>, GovernanceTagIntersectionSignal> {
    intersections
        .iter()
        .map(|intersection| (normalized_tags(&intersection.tags), intersection.clone()))
        .collect()
}

fn pair_count(
    pair_map: &BTreeMap<Vec<String>, GovernanceTagIntersectionSignal>,
    a: &str,
    b: &str,
) -> u64 {
    pair_map
        .get(&normalized_tags(&[a.to_string(), b.to_string()]))
        .map(|pair| pair.concept_entity_pages)
        .unwrap_or(0)
}

fn pair_page_union(
    pair_map: &BTreeMap<Vec<String>, GovernanceTagIntersectionSignal>,
    tags: &[String],
) -> Vec<String> {
    let mut page_ids = Vec::new();
    for i in 0..tags.len() {
        for b in tags.iter().skip(i + 1) {
            if let Some(pair) = pair_map.get(&normalized_tags(&[tags[i].clone(), b.clone()])) {
                page_ids.extend(pair.page_ids.clone());
            }
        }
    }
    dedupe_sorted(page_ids)
}

fn page_union_for_tags(
    tag_map: &BTreeMap<String, GovernanceTagSignal>,
    tags: &[String],
) -> Vec<String> {
    let mut page_ids = Vec::new();
    for tag in tags {
        if let Some(signal) = tag_map.get(tag) {
            page_ids.extend(signal.page_ids.clone());
        }
    }
    dedupe_sorted(page_ids)
}

fn source_domains_for_tags(
    tag_map: &BTreeMap<String, GovernanceTagSignal>,
    tags: &[String],
) -> Vec<String> {
    let mut domains = Vec::new();
    for tag in tags {
        if let Some(signal) = tag_map.get(tag) {
            domains.extend(signal.source_domains.clone());
        }
    }
    dedupe_sorted(domains)
}

fn anchor_with_d(anchor: &[String], d: &str) -> Vec<String> {
    let mut tags = anchor.to_vec();
    tags.push(d.to_string());
    tags.sort();
    tags
}

fn normalized_tags(tags: &[String]) -> Vec<String> {
    tags.iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn dedupe_sorted(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{
        GovernanceScanSummary, GovernanceSynthesisSignals, GovernanceSynthesisTopicSignal,
    };

    fn tag(name: &str, count: usize) -> GovernanceTagSignal {
        GovernanceTagSignal {
            tag: name.into(),
            concept_entity_pages: count as u64,
            page_ids: (0..count).map(|idx| format!("{name}-p{idx}")).collect(),
            source_domains: vec![format!("{name}.example")],
        }
    }

    fn pair(a: &str, b: &str, count: usize) -> GovernanceTagIntersectionSignal {
        GovernanceTagIntersectionSignal {
            tags: normalized_tags(&[a.into(), b.into()]),
            concept_entity_pages: count as u64,
            page_ids: (0..count).map(|idx| format!("{a}-{b}-p{idx}")).collect(),
        }
    }

    fn scan(
        tags: Vec<GovernanceTagSignal>,
        intersections: Vec<GovernanceTagIntersectionSignal>,
        existing_topics: Vec<GovernanceSynthesisTopicSignal>,
    ) -> GovernanceScanReport {
        GovernanceScanReport {
            report_id: "scan-1".into(),
            generated_at: None,
            viewer_scope: Some("shared:wiki".into()),
            summary: GovernanceScanSummary::default(),
            lifecycle: Vec::new(),
            references: Vec::new(),
            lint: Vec::new(),
            gaps: Vec::new(),
            duplicates: Vec::new(),
            retire_candidates: Vec::new(),
            synthesis_signals: GovernanceSynthesisSignals {
                tags,
                intersections,
                deprecated_tags_used: Vec::new(),
                existing_topics,
            },
        }
    }

    fn options() -> SynthesisDiscoveryOptions {
        SynthesisDiscoveryOptions {
            generated_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            report_id: "synthesis-discovery-1".into(),
            max_single_double: 10,
            max_triple: 10,
            max_quad: 10,
        }
    }

    #[test]
    fn discovers_single_and_double_thresholds() {
        let report = discover_synthesis_candidates(
            &scan(
                vec![tag("alpha", 10), tag("beta", 5)],
                vec![pair("alpha", "beta", 3)],
                Vec::new(),
            ),
            options(),
        );
        assert!(report
            .candidates
            .iter()
            .any(|c| c.kind == SynthesisCandidateKind::SingleTag
                && c.tags == vec!["alpha".to_string()]));
        assert!(report
            .candidates
            .iter()
            .any(|c| c.kind == SynthesisCandidateKind::DoubleTag
                && c.tags == vec!["alpha".to_string(), "beta".to_string()]));
    }

    #[test]
    fn triple_uses_pairwise_triangle_not_triple_intersection() {
        let report = discover_synthesis_candidates(
            &scan(
                vec![tag("a", 5), tag("b", 5), tag("c", 5)],
                vec![pair("a", "b", 2), pair("a", "c", 2), pair("b", "c", 2)],
                Vec::new(),
            ),
            options(),
        );
        let triple = report
            .candidates
            .iter()
            .find(|candidate| candidate.kind == SynthesisCandidateKind::TripleTag)
            .unwrap();
        assert_eq!(
            triple.tags,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(triple.concept_entity_pages, 6);
    }

    #[test]
    fn quad_requires_existing_anchor_and_distant_fourth_tag() {
        let existing_topics = vec![
            GovernanceSynthesisTopicSignal {
                page_id: "s-ab".into(),
                title: "ab".into(),
                tags: vec!["a".into(), "b".into()],
                status: None,
                source_domains: Vec::new(),
            },
            GovernanceSynthesisTopicSignal {
                page_id: "s-ac".into(),
                title: "ac".into(),
                tags: vec!["a".into(), "c".into()],
                status: None,
                source_domains: Vec::new(),
            },
        ];
        let report = discover_synthesis_candidates(
            &scan(
                vec![tag("a", 5), tag("b", 5), tag("c", 5), tag("d", 5)],
                vec![
                    pair("a", "b", 2),
                    pair("a", "c", 2),
                    pair("b", "c", 2),
                    pair("a", "d", 3),
                    pair("b", "d", 1),
                    pair("c", "d", 0),
                ],
                existing_topics,
            ),
            options(),
        );
        let quad = report
            .candidates
            .iter()
            .find(|candidate| candidate.kind == SynthesisCandidateKind::QuadTag)
            .unwrap();
        assert_eq!(
            quad.anchor_tags,
            vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
        );
        assert_eq!(
            quad.tags,
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string()
            ]
        );
    }

    #[test]
    fn quad_rejects_close_fourth_tag() {
        let existing_topics = vec![GovernanceSynthesisTopicSignal {
            page_id: "s-abc".into(),
            title: "abc".into(),
            tags: vec!["a".into(), "b".into(), "c".into()],
            status: None,
            source_domains: Vec::new(),
        }];
        let report = discover_synthesis_candidates(
            &scan(
                vec![tag("a", 5), tag("b", 5), tag("c", 5), tag("d", 5)],
                vec![
                    pair("a", "b", 2),
                    pair("a", "c", 2),
                    pair("b", "c", 2),
                    pair("a", "d", 4),
                ],
                existing_topics,
            ),
            options(),
        );
        assert!(!report
            .candidates
            .iter()
            .any(|candidate| candidate.kind == SynthesisCandidateKind::QuadTag));
    }

    #[test]
    fn applies_round_limits_and_skips_covered_topics() {
        let mut options = options();
        options.max_single_double = 1;
        options.max_triple = 1;
        options.max_quad = 0;
        let report = discover_synthesis_candidates(
            &scan(
                vec![tag("a", 10), tag("b", 10), tag("c", 10), tag("covered", 10)],
                vec![pair("a", "b", 3), pair("a", "c", 2), pair("b", "c", 2)],
                vec![GovernanceSynthesisTopicSignal {
                    page_id: "s-covered".into(),
                    title: "covered".into(),
                    tags: vec!["covered".into()],
                    status: None,
                    source_domains: Vec::new(),
                }],
            ),
            options,
        );
        assert_eq!(report.summary.triple_tag, 1);
        assert_eq!(report.summary.single_tag + report.summary.double_tag, 1);
        assert!(!report
            .candidates
            .iter()
            .any(|candidate| candidate.tags == vec!["covered".to_string()]));
    }
}
