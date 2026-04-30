//! Synthesis discovery report model.
//!
//! These structs are pure data contracts. Discovery logic and IO live in higher
//! layers.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynthesisCandidateKind {
    SingleTag,
    DoubleTag,
    TripleTag,
    QuadTag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SynthesisDiscoverySummary {
    pub total: u64,
    pub single_tag: u64,
    pub double_tag: u64,
    pub triple_tag: u64,
    pub quad_tag: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthesisCandidate {
    pub candidate_id: String,
    pub kind: SynthesisCandidateKind,
    pub tags: Vec<String>,
    pub anchor_tags: Vec<Vec<String>>,
    pub concept_entity_pages: u64,
    pub page_ids: Vec<String>,
    pub source_domains: Vec<String>,
    pub score: i64,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthesisDiscoveryReport {
    pub report_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub viewer_scope: Option<String>,
    pub source_scan_report_id: String,
    pub summary: SynthesisDiscoverySummary,
    pub candidates: Vec<SynthesisCandidate>,
}

impl SynthesisDiscoveryReport {
    pub fn new(
        report_id: impl Into<String>,
        generated_at: OffsetDateTime,
        viewer_scope: Option<String>,
        source_scan_report_id: impl Into<String>,
    ) -> Self {
        Self {
            report_id: report_id.into(),
            generated_at: Some(generated_at),
            viewer_scope,
            source_scan_report_id: source_scan_report_id.into(),
            summary: SynthesisDiscoverySummary::default(),
            candidates: Vec::new(),
        }
    }

    pub fn refresh_summary(&mut self) {
        let mut summary = SynthesisDiscoverySummary {
            total: self.candidates.len() as u64,
            ..SynthesisDiscoverySummary::default()
        };
        for candidate in &self.candidates {
            match candidate.kind {
                SynthesisCandidateKind::SingleTag => summary.single_tag += 1,
                SynthesisCandidateKind::DoubleTag => summary.double_tag += 1,
                SynthesisCandidateKind::TripleTag => summary.triple_tag += 1,
                SynthesisCandidateKind::QuadTag => summary.quad_tag += 1,
            }
        }
        self.summary = summary;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_stable_shape() {
        let mut report = SynthesisDiscoveryReport {
            report_id: "synth-1".into(),
            generated_at: None,
            viewer_scope: Some("shared:wiki".into()),
            source_scan_report_id: "scan-1".into(),
            summary: SynthesisDiscoverySummary::default(),
            candidates: vec![SynthesisCandidate {
                candidate_id: "synth-cand-0001".into(),
                kind: SynthesisCandidateKind::TripleTag,
                tags: vec!["a".into(), "b".into(), "c".into()],
                anchor_tags: Vec::new(),
                concept_entity_pages: 6,
                page_ids: vec!["p1".into()],
                source_domains: vec!["example.com".into()],
                score: 3_006_001,
                rationale: "test".into(),
            }],
        };
        report.refresh_summary();
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["report_id"], "synth-1");
        assert_eq!(value["summary"]["triple_tag"], 1);
        assert_eq!(value["candidates"][0]["kind"], "triple_tag");
    }
}
