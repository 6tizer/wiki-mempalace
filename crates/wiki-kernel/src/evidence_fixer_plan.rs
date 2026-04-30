use std::collections::BTreeSet;

use time::OffsetDateTime;
use wiki_core::{
    EvidenceFixAction, EvidenceFixActionKind, EvidenceFixActionStatus, EvidenceFixEvidence,
    EvidenceFixPayload, EvidenceFixerPlan, GovernanceDuplicateGroup, GovernanceScanReport,
    SemanticPatchProposal,
};

pub struct EvidenceFixerPlanOptions {
    pub generated_at: OffsetDateTime,
    pub plan_id: String,
    pub web_available: bool,
    pub web_cross_verified_keys: BTreeSet<String>,
    pub semantic_patch_proposals: Vec<SemanticPatchProposal>,
}

pub fn build_evidence_fixer_plan(
    scan: &GovernanceScanReport,
    options: EvidenceFixerPlanOptions,
) -> EvidenceFixerPlan {
    let mut builder = PlanBuilder::new(scan, options);
    builder.add_lifecycle_actions();
    builder.add_lint_actions();
    builder.add_reference_actions();
    builder.add_duplicate_actions();
    builder.add_retire_actions();
    builder.add_deprecated_tag_actions();
    builder.add_semantic_patch_actions();
    builder.finish()
}

pub fn duplicate_web_verification_key(group: &GovernanceDuplicateGroup) -> String {
    format!(
        "duplicate:{}:{}",
        group.kind,
        group
            .members
            .iter()
            .map(|member| member.subject_id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )
}

struct PlanBuilder<'a> {
    scan: &'a GovernanceScanReport,
    options: EvidenceFixerPlanOptions,
    plan: EvidenceFixerPlan,
    next_id: usize,
}

struct ActionDraft {
    kind: EvidenceFixActionKind,
    subject_type: String,
    subject_id: Option<String>,
    label: Option<String>,
    evidence: Vec<EvidenceFixEvidence>,
    payload: EvidenceFixPayload,
}

impl<'a> PlanBuilder<'a> {
    fn new(scan: &'a GovernanceScanReport, options: EvidenceFixerPlanOptions) -> Self {
        let plan = EvidenceFixerPlan::new(
            options.plan_id.clone(),
            scan.report_id.clone(),
            options.generated_at,
            scan.viewer_scope.clone(),
        );
        Self {
            scan,
            options,
            plan,
            next_id: 1,
        }
    }

    fn finish(mut self) -> EvidenceFixerPlan {
        self.plan.refresh_summary();
        self.plan
    }

    fn action(
        &mut self,
        draft: ActionDraft,
        status: EvidenceFixActionStatus,
        blockers: Vec<String>,
    ) {
        let action_id = format!("fix-{:04}", self.next_id);
        self.next_id += 1;
        self.plan.actions.push(EvidenceFixAction {
            action_id,
            kind: draft.kind,
            status,
            subject_type: draft.subject_type,
            subject_id: draft.subject_id,
            label: draft.label,
            evidence: draft.evidence,
            blockers,
            payload: draft.payload,
        });
    }

    fn ready(
        &mut self,
        kind: EvidenceFixActionKind,
        subject_type: impl Into<String>,
        subject_id: Option<String>,
        label: Option<String>,
        evidence: Vec<EvidenceFixEvidence>,
        payload: EvidenceFixPayload,
    ) {
        self.action(
            ActionDraft {
                kind,
                subject_type: subject_type.into(),
                subject_id,
                label,
                evidence,
                payload,
            },
            EvidenceFixActionStatus::Ready,
            Vec::new(),
        );
    }

    fn blocked(&mut self, draft: ActionDraft, blockers: Vec<String>) {
        self.action(draft, EvidenceFixActionStatus::Blocked, blockers);
    }

    fn add_lifecycle_actions(&mut self) {
        for signal in &self.scan.lifecycle {
            if signal.subject_type == "page" && signal.eligible {
                if let Some(target_status) = signal.next_status {
                    self.ready(
                        EvidenceFixActionKind::PromoteStatus,
                        "page",
                        Some(signal.subject_id.clone()),
                        signal.label.clone(),
                        vec![evidence("governance.lifecycle", "promotion blockers empty")],
                        EvidenceFixPayload::PromoteStatus { target_status },
                    );
                }
            }
        }
    }

    fn add_lint_actions(&mut self) {
        for finding in &self.scan.lint {
            match finding.code.as_str() {
                "page.incomplete" => {
                    let section = section_from_incomplete_message(&finding.message)
                        .unwrap_or_else(|| "待补充段落".to_string());
                    self.ready(
                        EvidenceFixActionKind::AddMissingSection,
                        "page",
                        finding.subject.clone(),
                        None,
                        vec![evidence("lint", &finding.message)],
                        EvidenceFixPayload::AddMissingSection {
                            sections: vec![section],
                        },
                    );
                }
                "page.empty_title" => {
                    self.ready(
                        EvidenceFixActionKind::SetTitleFromH1,
                        "page",
                        finding.subject.clone(),
                        None,
                        vec![evidence("lint", &finding.message)],
                        EvidenceFixPayload::SetTitleFromH1 { title: None },
                    );
                }
                _ => {}
            }
        }
    }

    fn add_reference_actions(&mut self) {
        for finding in &self.scan.references {
            match finding.code.as_str() {
                "reference.duplicate_source_id" => self.ready(
                    EvidenceFixActionKind::DedupeSourceReference,
                    finding.subject_type.clone(),
                    finding.subject_id.clone(),
                    finding.label.clone(),
                    vec![evidence("reference", &finding.message)],
                    EvidenceFixPayload::DedupeSourceReference,
                ),
                "reference.raw_url" | "reference.duplicate_raw_url" => self.blocked(
                    ActionDraft {
                        kind: EvidenceFixActionKind::UpgradeSourceReference,
                        subject_type: finding.subject_type.clone(),
                        subject_id: finding.subject_id.clone(),
                        label: finding.label.clone(),
                        evidence: vec![evidence("reference", &finding.message)],
                        payload: EvidenceFixPayload::UpgradeSourceReference {
                            raw_url: (finding.code == "reference.duplicate_raw_url")
                                .then(|| finding.label.clone())
                                .flatten(),
                        },
                    },
                    vec!["requires_exact_raw_url_and_current_page_recheck".into()],
                ),
                _ => {}
            }
        }
    }

    fn add_duplicate_actions(&mut self) {
        for group in &self.scan.duplicates {
            let member_ids = group
                .members
                .iter()
                .map(|member| member.subject_id.clone())
                .collect::<Vec<_>>();
            let payload = EvidenceFixPayload::MergeDuplicate {
                duplicate_kind: group.kind.clone(),
                member_ids,
                confidence: group.confidence.clone(),
            };
            let evidence_items = vec![evidence(
                "duplicate",
                &format!("{} duplicate group", group.confidence),
            )];
            if group.confidence == "exact" {
                self.ready(
                    EvidenceFixActionKind::MergeDuplicate,
                    "duplicate_group",
                    None,
                    Some(group.kind.clone()),
                    evidence_items,
                    payload,
                );
                continue;
            }

            let key = duplicate_web_verification_key(group);
            if self.options.web_cross_verified_keys.contains(&key) {
                self.ready(
                    EvidenceFixActionKind::MergeDuplicate,
                    "duplicate_group",
                    None,
                    Some(group.kind.clone()),
                    vec![
                        evidence("duplicate", "near duplicate group"),
                        evidence("web_cross_verify", &key),
                    ],
                    payload,
                );
            } else {
                let blocker = if self.options.web_available {
                    "web_cross_verification_not_met"
                } else {
                    "web_cross_verification_required"
                };
                self.blocked(
                    ActionDraft {
                        kind: EvidenceFixActionKind::MergeDuplicate,
                        subject_type: "duplicate_group".into(),
                        subject_id: None,
                        label: Some(group.kind.clone()),
                        evidence: evidence_items,
                        payload,
                    },
                    vec![blocker.into()],
                );
            }
        }
    }

    fn add_retire_actions(&mut self) {
        for candidate in &self.scan.retire_candidates {
            self.ready(
                EvidenceFixActionKind::RetirePage,
                "page",
                Some(candidate.page_id.clone()),
                Some(candidate.title.clone()),
                vec![evidence("retire_candidate", &candidate.reason)],
                EvidenceFixPayload::RetirePage {
                    tombstone_required: true,
                },
            );
        }
    }

    fn add_deprecated_tag_actions(&mut self) {
        for tag in &self.scan.synthesis_signals.deprecated_tags_used {
            self.blocked(
                ActionDraft {
                    kind: EvidenceFixActionKind::ReplaceDeprecatedTag,
                    subject_type: "tag".into(),
                    subject_id: None,
                    label: Some(tag.clone()),
                    evidence: vec![evidence("deprecated_tag", tag)],
                    payload: EvidenceFixPayload::ReplaceDeprecatedTag {
                        tag: tag.clone(),
                        replacement: None,
                    },
                },
                vec!["replacement_tag_required".into()],
            );
        }
    }

    fn add_semantic_patch_actions(&mut self) {
        let proposals = self.options.semantic_patch_proposals.clone();
        for proposal in proposals {
            let evidence_items = proposal
                .source_ids
                .iter()
                .map(|source| evidence("source_id", source))
                .collect::<Vec<_>>();
            let payload = EvidenceFixPayload::SemanticPatch {
                old_text: proposal.old_text.clone(),
                new_text: proposal.new_text.clone(),
                target_range: proposal.target_range.clone(),
                source_ids: proposal.source_ids.clone(),
            };
            let mut blockers = Vec::new();
            if proposal.old_text.trim().is_empty()
                || proposal.new_text.trim().is_empty()
                || proposal.target_range.trim().is_empty()
            {
                blockers.push("patch_range_or_text_missing".to_string());
            }
            if proposal.source_ids.is_empty() {
                blockers.push("source_chain_required".to_string());
            }
            if !proposal.verifier_passed {
                blockers.push("verifier_not_passed".to_string());
            }
            if blockers.is_empty() {
                self.ready(
                    EvidenceFixActionKind::SemanticPatch,
                    proposal.subject_type,
                    Some(proposal.subject_id),
                    None,
                    evidence_items,
                    payload,
                );
            } else {
                self.blocked(
                    ActionDraft {
                        kind: EvidenceFixActionKind::SemanticPatch,
                        subject_type: proposal.subject_type,
                        subject_id: Some(proposal.subject_id),
                        label: None,
                        evidence: evidence_items,
                        payload,
                    },
                    blockers,
                );
            }
        }
    }
}

fn evidence(kind: &str, detail: &str) -> EvidenceFixEvidence {
    EvidenceFixEvidence {
        kind: kind.into(),
        detail: detail.into(),
    }
}

fn section_from_incomplete_message(message: &str) -> Option<String> {
    let marker = "页面缺少必需段落：";
    message.find(marker).and_then(|idx| {
        let section = message[idx + marker.len()..]
            .lines()
            .next()
            .unwrap_or_default()
            .trim();
        (!section.is_empty()).then(|| section.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{
        EntryStatus, GovernanceDuplicateGroup, GovernanceDuplicateMember,
        GovernanceLifecycleSignal, GovernanceLintFinding, GovernanceReferenceFinding,
        GovernanceRetireCandidate, GovernanceScanReport, GovernanceScanSummary,
        GovernanceSynthesisSignals, LintSeverity,
    };

    fn empty_scan() -> GovernanceScanReport {
        GovernanceScanReport {
            report_id: "scan-1".into(),
            generated_at: None,
            viewer_scope: Some("private:cli".into()),
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

    fn options() -> EvidenceFixerPlanOptions {
        EvidenceFixerPlanOptions {
            generated_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            plan_id: "plan-1".into(),
            web_available: false,
            web_cross_verified_keys: BTreeSet::new(),
            semantic_patch_proposals: Vec::new(),
        }
    }

    #[test]
    fn plan_builds_rule_determined_ready_actions() {
        let mut scan = empty_scan();
        scan.lifecycle.push(GovernanceLifecycleSignal {
            subject_type: "page".into(),
            subject_id: "p1".into(),
            label: Some("Page".into()),
            entry_type: None,
            status: None,
            tier: None,
            next_status: Some(EntryStatus::InReview),
            eligible: true,
            blockers: Vec::new(),
        });
        scan.lint.push(GovernanceLintFinding {
            code: "page.incomplete".into(),
            message: "页面缺少必需段落：来源引用".into(),
            severity: LintSeverity::Warn,
            subject: Some("p1".into()),
        });
        scan.references.push(GovernanceReferenceFinding {
            code: "reference.duplicate_source_id".into(),
            message: "duplicate".into(),
            severity: "info".into(),
            subject_type: "claim".into(),
            subject_id: Some("c1".into()),
            label: None,
        });

        let plan = build_evidence_fixer_plan(&scan, options());
        assert_eq!(plan.summary.ready, 3);
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == EvidenceFixActionKind::PromoteStatus));
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == EvidenceFixActionKind::AddMissingSection));
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == EvidenceFixActionKind::DedupeSourceReference));
    }

    #[test]
    fn near_duplicate_blocks_without_web_cross_verification() {
        let mut scan = empty_scan();
        scan.duplicates.push(GovernanceDuplicateGroup {
            kind: "claim_text".into(),
            key: "near".into(),
            confidence: "near".into(),
            members: vec![
                GovernanceDuplicateMember {
                    subject_type: "claim".into(),
                    subject_id: "c1".into(),
                    label: None,
                },
                GovernanceDuplicateMember {
                    subject_type: "claim".into(),
                    subject_id: "c2".into(),
                    label: None,
                },
            ],
        });

        let plan = build_evidence_fixer_plan(&scan, options());
        assert_eq!(plan.summary.blocked, 1);
        assert_eq!(
            plan.actions[0].blockers,
            vec!["web_cross_verification_required"]
        );
    }

    #[test]
    fn near_duplicate_ready_with_fake_web_cross_verification() {
        let mut scan = empty_scan();
        let group = GovernanceDuplicateGroup {
            kind: "claim_text".into(),
            key: "near".into(),
            confidence: "near".into(),
            members: vec![
                GovernanceDuplicateMember {
                    subject_type: "claim".into(),
                    subject_id: "c1".into(),
                    label: None,
                },
                GovernanceDuplicateMember {
                    subject_type: "claim".into(),
                    subject_id: "c2".into(),
                    label: None,
                },
            ],
        };
        let key = duplicate_web_verification_key(&group);
        scan.duplicates.push(group);
        let mut opts = options();
        opts.web_available = true;
        opts.web_cross_verified_keys.insert(key);

        let plan = build_evidence_fixer_plan(&scan, opts);
        assert_eq!(plan.summary.ready, 1);
        assert!(plan.actions[0].blockers.is_empty());
    }

    #[test]
    fn semantic_patch_without_source_is_blocked() {
        let scan = empty_scan();
        let mut opts = options();
        opts.semantic_patch_proposals.push(SemanticPatchProposal {
            subject_type: "page".into(),
            subject_id: "p1".into(),
            old_text: "old".into(),
            new_text: "new".into(),
            target_range: "1:1".into(),
            source_ids: Vec::new(),
            verifier_passed: true,
        });

        let plan = build_evidence_fixer_plan(&scan, opts);
        assert_eq!(plan.summary.blocked, 1);
        assert_eq!(plan.actions[0].blockers, vec!["source_chain_required"]);
    }

    #[test]
    fn semantic_patch_with_source_and_verifier_is_ready() {
        let scan = empty_scan();
        let mut opts = options();
        opts.semantic_patch_proposals.push(SemanticPatchProposal {
            subject_type: "page".into(),
            subject_id: "p1".into(),
            old_text: "old".into(),
            new_text: "new".into(),
            target_range: "1:1".into(),
            source_ids: vec!["s1".into()],
            verifier_passed: true,
        });

        let plan = build_evidence_fixer_plan(&scan, opts);
        assert_eq!(plan.summary.ready, 1);
        assert_eq!(plan.actions[0].kind, EvidenceFixActionKind::SemanticPatch);
    }

    #[test]
    fn retire_candidates_require_tombstone_snapshot() {
        let mut scan = empty_scan();
        scan.retire_candidates.push(GovernanceRetireCandidate {
            page_id: "p1".into(),
            title: "Empty".into(),
            reason: "empty projection page".into(),
            evidence: vec!["no_claims".into()],
        });

        let plan = build_evidence_fixer_plan(&scan, options());
        assert_eq!(plan.summary.ready, 1);
        assert_eq!(plan.actions[0].kind, EvidenceFixActionKind::RetirePage);
        assert_eq!(
            plan.actions[0].payload,
            EvidenceFixPayload::RetirePage {
                tombstone_required: true
            }
        );
    }
}
