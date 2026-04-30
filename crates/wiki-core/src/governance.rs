//! Unified governance scan report model.
//!
//! These structs are pure data contracts. Scanning and IO live in higher layers.

use crate::gap::GapSeverity;
use crate::quality::LintSeverity;
use crate::schema::{EntryStatus, EntryType};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GovernanceScanSummary {
    pub lifecycle_total: u64,
    pub lifecycle_promotable: u64,
    pub lifecycle_blocked: u64,
    pub references_total: u64,
    pub lint_total: u64,
    pub gaps_total: u64,
    pub duplicate_groups_total: u64,
    pub retire_candidates_total: u64,
    pub tag_signal_total: u64,
    pub tag_intersection_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceScanReport {
    pub report_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub viewer_scope: Option<String>,
    pub summary: GovernanceScanSummary,
    pub lifecycle: Vec<GovernanceLifecycleSignal>,
    pub references: Vec<GovernanceReferenceFinding>,
    pub lint: Vec<GovernanceLintFinding>,
    pub gaps: Vec<GovernanceGapFinding>,
    pub duplicates: Vec<GovernanceDuplicateGroup>,
    pub retire_candidates: Vec<GovernanceRetireCandidate>,
    pub synthesis_signals: GovernanceSynthesisSignals,
}

impl GovernanceScanReport {
    pub fn new(
        report_id: impl Into<String>,
        generated_at: OffsetDateTime,
        viewer_scope: Option<String>,
    ) -> Self {
        Self {
            report_id: report_id.into(),
            generated_at: Some(generated_at),
            viewer_scope,
            summary: GovernanceScanSummary::default(),
            lifecycle: Vec::new(),
            references: Vec::new(),
            lint: Vec::new(),
            gaps: Vec::new(),
            duplicates: Vec::new(),
            retire_candidates: Vec::new(),
            synthesis_signals: GovernanceSynthesisSignals::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceLifecycleSignal {
    pub subject_type: String,
    pub subject_id: String,
    pub label: Option<String>,
    pub entry_type: Option<EntryType>,
    pub status: Option<EntryStatus>,
    pub tier: Option<String>,
    pub next_status: Option<EntryStatus>,
    pub eligible: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceReferenceFinding {
    pub code: String,
    pub message: String,
    pub severity: String,
    pub subject_type: String,
    pub subject_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceLintFinding {
    pub code: String,
    pub message: String,
    pub severity: LintSeverity,
    pub subject: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceGapFinding {
    pub code: String,
    pub message: String,
    pub severity: GapSeverity,
    pub subject: Option<String>,
    pub subject_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceDuplicateMember {
    pub subject_type: String,
    pub subject_id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceDuplicateGroup {
    pub kind: String,
    pub key: String,
    pub confidence: String,
    pub members: Vec<GovernanceDuplicateMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceRetireCandidate {
    pub page_id: String,
    pub title: String,
    pub reason: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GovernanceSynthesisSignals {
    pub tags: Vec<GovernanceTagSignal>,
    pub intersections: Vec<GovernanceTagIntersectionSignal>,
    pub deprecated_tags_used: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceTagSignal {
    pub tag: String,
    pub concept_entity_pages: u64,
    pub page_ids: Vec<String>,
    pub source_domains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceTagIntersectionSignal {
    pub tags: Vec<String>,
    pub concept_entity_pages: u64,
    pub page_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_stable_shape() {
        let report = GovernanceScanReport {
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
            synthesis_signals: GovernanceSynthesisSignals::default(),
        };
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["report_id"], "scan-1");
        assert!(value["summary"]["lint_total"].is_number());
        assert!(value["synthesis_signals"]["tags"].is_array());
    }
}
