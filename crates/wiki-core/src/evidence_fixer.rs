//! Evidence Fixer typed dry-run plan.

use crate::{
    model::Claim,
    page::WikiPage,
    schema::{EntryStatus, EntryType},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceFixerPlanSummary {
    pub total: u64,
    pub ready: u64,
    pub blocked: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFixerPlan {
    pub plan_id: String,
    pub source_scan_report_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub viewer_scope: Option<String>,
    pub summary: EvidenceFixerPlanSummary,
    pub actions: Vec<EvidenceFixAction>,
}

impl EvidenceFixerPlan {
    pub fn new(
        plan_id: impl Into<String>,
        source_scan_report_id: impl Into<String>,
        generated_at: OffsetDateTime,
        viewer_scope: Option<String>,
    ) -> Self {
        Self {
            plan_id: plan_id.into(),
            source_scan_report_id: source_scan_report_id.into(),
            generated_at: Some(generated_at),
            viewer_scope,
            summary: EvidenceFixerPlanSummary::default(),
            actions: Vec::new(),
        }
    }

    pub fn refresh_summary(&mut self) {
        self.summary.total = self.actions.len() as u64;
        self.summary.ready = self
            .actions
            .iter()
            .filter(|action| action.status == EvidenceFixActionStatus::Ready)
            .count() as u64;
        self.summary.blocked = self.summary.total - self.summary.ready;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFixActionKind {
    PromoteStatus,
    UpgradeSourceReference,
    ReplaceDeprecatedTag,
    CorrectEntryType,
    MergeDuplicate,
    SemanticPatch,
    RetirePage,
    AddMissingSection,
    DedupeSourceReference,
    SetTitleFromH1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFixActionStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFixEvidence {
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFixAction {
    pub action_id: String,
    pub kind: EvidenceFixActionKind,
    pub status: EvidenceFixActionStatus,
    pub subject_type: String,
    pub subject_id: Option<String>,
    pub label: Option<String>,
    pub evidence: Vec<EvidenceFixEvidence>,
    pub blockers: Vec<String>,
    pub payload: EvidenceFixPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceFixPayload {
    PromoteStatus {
        target_status: EntryStatus,
    },
    UpgradeSourceReference {
        raw_url: Option<String>,
    },
    ReplaceDeprecatedTag {
        tag: String,
        replacement: Option<String>,
    },
    CorrectEntryType {
        suggested_type: Option<EntryType>,
    },
    MergeDuplicate {
        duplicate_kind: String,
        member_ids: Vec<String>,
        confidence: String,
    },
    SemanticPatch {
        old_text: String,
        new_text: String,
        target_range: String,
        source_ids: Vec<String>,
    },
    RetirePage {
        tombstone_required: bool,
    },
    AddMissingSection {
        sections: Vec<String>,
    },
    DedupeSourceReference,
    SetTitleFromH1 {
        title: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticPatchProposal {
    pub subject_type: String,
    pub subject_id: String,
    pub old_text: String,
    pub new_text: String,
    pub target_range: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub verifier_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticPatchProposalSet {
    #[serde(default)]
    pub proposals: Vec<SemanticPatchProposal>,
}

pub fn parse_semantic_patch_proposals_json(
    raw: &str,
) -> Result<Vec<SemanticPatchProposal>, serde_json::Error> {
    serde_json::from_str::<SemanticPatchProposalSet>(raw).map(|set| set.proposals)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFixerApplyPolicy {
    EvidenceAuto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFixerRunMode {
    Preflight,
    Apply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFixerApplyActionStatus {
    WouldApply,
    Applied,
    Blocked,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvidenceFixerApplySummary {
    pub total: u64,
    pub would_apply: u64,
    pub applied: u64,
    pub blocked: u64,
    pub skipped: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFixerApplyActionReport {
    pub action_id: String,
    pub kind: EvidenceFixActionKind,
    pub subject_type: String,
    pub subject_id: Option<String>,
    pub status: EvidenceFixerApplyActionStatus,
    pub reason: String,
    pub tombstone_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceFixerApplyReport {
    pub report_id: String,
    pub plan_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub mode: EvidenceFixerRunMode,
    pub policy: EvidenceFixerApplyPolicy,
    pub summary: EvidenceFixerApplySummary,
    pub actions: Vec<EvidenceFixerApplyActionReport>,
    pub tombstones: Vec<EvidenceFixerTombstone>,
}

impl EvidenceFixerApplyReport {
    pub fn refresh_summary(&mut self) {
        self.summary.total = self.actions.len() as u64;
        self.summary.would_apply = self
            .actions
            .iter()
            .filter(|action| action.status == EvidenceFixerApplyActionStatus::WouldApply)
            .count() as u64;
        self.summary.applied = self
            .actions
            .iter()
            .filter(|action| action.status == EvidenceFixerApplyActionStatus::Applied)
            .count() as u64;
        self.summary.blocked = self
            .actions
            .iter()
            .filter(|action| action.status == EvidenceFixerApplyActionStatus::Blocked)
            .count() as u64;
        self.summary.skipped = self
            .actions
            .iter()
            .filter(|action| action.status == EvidenceFixerApplyActionStatus::Skipped)
            .count() as u64;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceFixerTombstone {
    pub tombstone_id: String,
    pub action_id: String,
    pub subject_type: String,
    pub subject_id: String,
    pub created_at: Option<OffsetDateTime>,
    pub snapshot: EvidenceFixerSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceFixerSnapshot {
    Page { page: WikiPage },
    Claim { claim: Claim },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceFixerRestoreReport {
    pub report_id: String,
    pub tombstone_id: String,
    pub generated_at: Option<OffsetDateTime>,
    pub mode: EvidenceFixerRunMode,
    pub status: EvidenceFixerApplyActionStatus,
    pub reason: String,
    pub restored_subject_type: String,
    pub restored_subject_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn illegal_semantic_patch_json_returns_error() {
        let err = parse_semantic_patch_proposals_json("{not-json").unwrap_err();
        assert!(err.is_syntax() || err.is_data());
    }

    #[test]
    fn semantic_patch_json_requires_expected_shape() {
        let raw = r#"{"proposals":[{"subject_type":"page","subject_id":"p1","old_text":"a","new_text":"b","target_range":"1:1","source_ids":["s1"],"verifier_passed":true}]}"#;
        let proposals = parse_semantic_patch_proposals_json(raw).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].source_ids, vec!["s1"]);
    }

    #[test]
    fn apply_report_summarizes_statuses() {
        let mut report = EvidenceFixerApplyReport {
            report_id: "r1".into(),
            plan_id: "p1".into(),
            generated_at: None,
            mode: EvidenceFixerRunMode::Preflight,
            policy: EvidenceFixerApplyPolicy::EvidenceAuto,
            summary: EvidenceFixerApplySummary::default(),
            actions: vec![EvidenceFixerApplyActionReport {
                action_id: "a1".into(),
                kind: EvidenceFixActionKind::RetirePage,
                subject_type: "page".into(),
                subject_id: None,
                status: EvidenceFixerApplyActionStatus::WouldApply,
                reason: "ok".into(),
                tombstone_id: None,
            }],
            tombstones: Vec::new(),
        };
        report.refresh_summary();
        assert_eq!(report.summary.total, 1);
        assert_eq!(report.summary.would_apply, 1);
    }
}
