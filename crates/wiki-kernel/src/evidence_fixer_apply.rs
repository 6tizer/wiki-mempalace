use time::OffsetDateTime;
use uuid::Uuid;
use wiki_core::{
    document_visible_to_viewer, Claim, ClaimId, EvidenceFixAction, EvidenceFixActionStatus,
    EvidenceFixPayload, EvidenceFixerApplyActionReport, EvidenceFixerApplyActionStatus,
    EvidenceFixerApplyPolicy, EvidenceFixerApplyReport, EvidenceFixerApplySummary,
    EvidenceFixerPlan, EvidenceFixerRestoreReport, EvidenceFixerRunMode, EvidenceFixerSnapshot,
    EvidenceFixerTombstone, PageId, Scope,
};

use crate::{LlmWikiEngine, NoopWikiHook};

pub struct EvidenceFixerApplyOptions {
    pub generated_at: OffsetDateTime,
    pub report_id: String,
    pub policy: EvidenceFixerApplyPolicy,
    pub apply: bool,
}

pub fn apply_evidence_fixer_plan(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    viewer: &Scope,
    plan: &EvidenceFixerPlan,
    options: EvidenceFixerApplyOptions,
) -> EvidenceFixerApplyReport {
    let mut report = EvidenceFixerApplyReport {
        report_id: options.report_id,
        plan_id: plan.plan_id.clone(),
        generated_at: Some(options.generated_at),
        mode: if options.apply {
            EvidenceFixerRunMode::Apply
        } else {
            EvidenceFixerRunMode::Preflight
        },
        policy: options.policy,
        summary: EvidenceFixerApplySummary::default(),
        actions: Vec::new(),
        tombstones: Vec::new(),
    };

    for action in &plan.actions {
        let (status, reason, tombstone) = apply_action(eng, viewer, action, options.apply);
        let tombstone_id = tombstone.as_ref().map(|item| item.tombstone_id.clone());
        if let Some(tombstone) = tombstone {
            report.tombstones.push(tombstone);
        }
        report.actions.push(EvidenceFixerApplyActionReport {
            action_id: action.action_id.clone(),
            kind: action.kind,
            subject_type: action.subject_type.clone(),
            subject_id: action.subject_id.clone(),
            status,
            reason,
            tombstone_id,
        });
    }

    report.refresh_summary();
    report
}

pub fn restore_evidence_fixer_tombstone(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    viewer: &Scope,
    tombstone: &EvidenceFixerTombstone,
    apply: bool,
    generated_at: OffsetDateTime,
    report_id: String,
) -> EvidenceFixerRestoreReport {
    let mode = if apply {
        EvidenceFixerRunMode::Apply
    } else {
        EvidenceFixerRunMode::Preflight
    };
    let (status, reason, restored_subject_type, restored_subject_id) = match &tombstone.snapshot {
        EvidenceFixerSnapshot::Page { page } => {
            if !document_visible_to_viewer(&page.scope, viewer) {
                (
                    EvidenceFixerApplyActionStatus::Blocked,
                    "tombstone page is outside viewer scope".to_string(),
                    "page".to_string(),
                    page.id.0.to_string(),
                )
            } else if apply {
                eng.write_page(page.clone(), "evidence-fixer-restore");
                (
                    EvidenceFixerApplyActionStatus::Applied,
                    "restored page snapshot".to_string(),
                    "page".to_string(),
                    page.id.0.to_string(),
                )
            } else {
                (
                    EvidenceFixerApplyActionStatus::WouldApply,
                    "would restore page snapshot".to_string(),
                    "page".to_string(),
                    page.id.0.to_string(),
                )
            }
        }
        EvidenceFixerSnapshot::Claim { claim } => {
            if !document_visible_to_viewer(&claim.scope, viewer) {
                (
                    EvidenceFixerApplyActionStatus::Blocked,
                    "tombstone claim is outside viewer scope".to_string(),
                    "claim".to_string(),
                    claim.id.0.to_string(),
                )
            } else if apply {
                eng.restore_claim_snapshot(claim.clone(), "evidence-fixer-restore");
                (
                    EvidenceFixerApplyActionStatus::Applied,
                    "restored claim snapshot".to_string(),
                    "claim".to_string(),
                    claim.id.0.to_string(),
                )
            } else {
                (
                    EvidenceFixerApplyActionStatus::WouldApply,
                    "would restore claim snapshot".to_string(),
                    "claim".to_string(),
                    claim.id.0.to_string(),
                )
            }
        }
    };

    EvidenceFixerRestoreReport {
        report_id,
        tombstone_id: tombstone.tombstone_id.clone(),
        generated_at: Some(generated_at),
        mode,
        status,
        reason,
        restored_subject_type,
        restored_subject_id,
    }
}

fn apply_action(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    viewer: &Scope,
    action: &EvidenceFixAction,
    apply: bool,
) -> (
    EvidenceFixerApplyActionStatus,
    String,
    Option<EvidenceFixerTombstone>,
) {
    if action.status != EvidenceFixActionStatus::Ready {
        return (
            EvidenceFixerApplyActionStatus::Blocked,
            "plan action is not ready".to_string(),
            None,
        );
    }

    match &action.payload {
        EvidenceFixPayload::PromoteStatus { target_status } => {
            let Some(page_id) = parse_page_subject(action) else {
                return blocked("missing or invalid page subject");
            };
            let Some(page) = eng.store.pages.get(&page_id) else {
                return skipped("page no longer exists");
            };
            if !document_visible_to_viewer(&page.scope, viewer) {
                return blocked("page is outside viewer scope");
            }
            if page.status == *target_status {
                return skipped("page is already at target status");
            }
            if apply {
                match eng.promote_page(
                    page_id,
                    *target_status,
                    "evidence-fixer",
                    OffsetDateTime::now_utc(),
                    false,
                ) {
                    Ok(()) => applied("promoted page status"),
                    Err(err) => blocked(&format!("promotion recheck failed: {err}")),
                }
            } else {
                would_apply("would promote page status")
            }
        }
        EvidenceFixPayload::AddMissingSection { sections } => {
            let Some(page_id) = parse_page_subject(action) else {
                return blocked("missing or invalid page subject");
            };
            let Some(page) = eng.store.pages.get(&page_id) else {
                return skipped("page no longer exists");
            };
            if !document_visible_to_viewer(&page.scope, viewer) {
                return blocked("page is outside viewer scope");
            }
            let missing = sections
                .iter()
                .filter(|section| !page.markdown.contains(&format!("## {section}")))
                .cloned()
                .collect::<Vec<_>>();
            if missing.is_empty() {
                return skipped("required sections already exist");
            }
            if apply {
                let mut next = page.clone();
                for section in missing {
                    next.markdown
                        .push_str(&format!("## {section}\n\n（待补充）\n\n"));
                }
                next.updated_at = OffsetDateTime::now_utc();
                next.refresh_outbound_links();
                eng.write_page(next, "evidence-fixer");
                applied("added missing sections")
            } else {
                would_apply("would add missing sections")
            }
        }
        EvidenceFixPayload::SetTitleFromH1 { title } => {
            let Some(page_id) = parse_page_subject(action) else {
                return blocked("missing or invalid page subject");
            };
            let Some(page) = eng.store.pages.get(&page_id) else {
                return skipped("page no longer exists");
            };
            if !document_visible_to_viewer(&page.scope, viewer) {
                return blocked("page is outside viewer scope");
            }
            if !page.title.trim().is_empty() {
                return skipped("page already has title");
            }
            let Some(title) = title
                .clone()
                .or_else(|| first_markdown_title(&page.markdown))
            else {
                return blocked("no title candidate found");
            };
            if apply {
                let mut next = page.clone();
                next.title = title;
                next.updated_at = OffsetDateTime::now_utc();
                eng.write_page(next, "evidence-fixer");
                applied("set title from heading")
            } else {
                would_apply("would set title from heading")
            }
        }
        EvidenceFixPayload::DedupeSourceReference => {
            let Some(claim_id) = parse_claim_subject(action) else {
                return blocked("missing or invalid claim subject");
            };
            let Some(claim) = eng.store.claims.get_mut(&claim_id) else {
                return skipped("claim no longer exists");
            };
            if !document_visible_to_viewer(&claim.scope, viewer) {
                return blocked("claim is outside viewer scope");
            }
            let before = claim.source_ids.len();
            if apply {
                claim.source_ids.sort_by_key(|source_id| source_id.0);
                claim.source_ids.dedup();
            }
            let after = if apply {
                claim.source_ids.len()
            } else {
                let mut ids = claim.source_ids.clone();
                ids.sort_by_key(|source_id| source_id.0);
                ids.dedup();
                ids.len()
            };
            if before == after {
                skipped("source references already deduped")
            } else if apply {
                applied("deduped claim source references")
            } else {
                would_apply("would dedupe claim source references")
            }
        }
        EvidenceFixPayload::MergeDuplicate {
            member_ids,
            confidence,
            ..
        } => merge_duplicate_claims(eng, viewer, action, member_ids, confidence, apply),
        EvidenceFixPayload::SemanticPatch {
            old_text,
            new_text,
            target_range,
            ..
        } => semantic_patch_page(eng, viewer, action, old_text, new_text, target_range, apply),
        EvidenceFixPayload::RetirePage { tombstone_required } => {
            if !*tombstone_required {
                return blocked("retire action lacks tombstone requirement");
            }
            let Some(page_id) = parse_page_subject(action) else {
                return blocked("missing or invalid page subject");
            };
            let Some(page) = eng.store.pages.get(&page_id) else {
                return skipped("page no longer exists");
            };
            if !document_visible_to_viewer(&page.scope, viewer) {
                return blocked("page is outside viewer scope");
            }
            let tombstone = page_tombstone(action, page.clone());
            if apply {
                eng.delete_page(page_id, "evidence-fixer");
                (
                    EvidenceFixerApplyActionStatus::Applied,
                    "retired page with tombstone".to_string(),
                    Some(tombstone),
                )
            } else {
                (
                    EvidenceFixerApplyActionStatus::WouldApply,
                    "would retire page with tombstone".to_string(),
                    Some(tombstone),
                )
            }
        }
        EvidenceFixPayload::UpgradeSourceReference { .. }
        | EvidenceFixPayload::ReplaceDeprecatedTag { .. }
        | EvidenceFixPayload::CorrectEntryType { .. } => {
            blocked("action kind is not executable in evidence-auto policy")
        }
    }
}

fn merge_duplicate_claims(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    viewer: &Scope,
    action: &EvidenceFixAction,
    member_ids: &[String],
    confidence: &str,
    apply: bool,
) -> (
    EvidenceFixerApplyActionStatus,
    String,
    Option<EvidenceFixerTombstone>,
) {
    if confidence != "exact" && !has_web_evidence(action) {
        return blocked("near duplicate merge lacks web cross verification evidence");
    }
    let claim_ids = member_ids
        .iter()
        .filter_map(|id| Uuid::parse_str(id).ok().map(ClaimId))
        .collect::<Vec<_>>();
    if claim_ids.len() < 2 || claim_ids.len() != member_ids.len() {
        return blocked("merge_duplicate currently requires at least two claim ids");
    }
    let canonical_id = claim_ids[0];
    let Some(canonical) = eng.store.claims.get(&canonical_id) else {
        return skipped("canonical claim no longer exists");
    };
    if !document_visible_to_viewer(&canonical.scope, viewer) {
        return blocked("canonical claim is outside viewer scope");
    }
    let mut merged_sources = canonical.source_ids.clone();
    let mut merged_tags = canonical.tags.clone();
    let mut removed = Vec::new();
    for claim_id in claim_ids.iter().skip(1) {
        let Some(claim) = eng.store.claims.get(claim_id) else {
            return skipped("duplicate claim no longer exists");
        };
        if !document_visible_to_viewer(&claim.scope, viewer) {
            return blocked("duplicate claim is outside viewer scope");
        }
        merged_sources.extend(claim.source_ids.iter().copied());
        merged_tags.extend(claim.tags.iter().cloned());
        removed.push((*claim_id, claim.clone()));
    }
    merged_sources.sort_by_key(|source_id| source_id.0);
    merged_sources.dedup();
    merged_tags.sort();
    merged_tags.dedup();
    if apply {
        if let Some(canonical) = eng.store.claims.get_mut(&canonical_id) {
            canonical.source_ids = merged_sources;
            canonical.tags = merged_tags;
        }
        let mut last_tombstone = None;
        for (claim_id, claim) in removed {
            let tombstone = claim_tombstone(action, claim);
            eng.delete_claim(claim_id, "evidence-fixer");
            last_tombstone = Some(tombstone);
        }
        (
            EvidenceFixerApplyActionStatus::Applied,
            "merged duplicate claims without dropping source refs".to_string(),
            last_tombstone,
        )
    } else {
        would_apply("would merge duplicate claims")
    }
}

fn semantic_patch_page(
    eng: &mut LlmWikiEngine<NoopWikiHook>,
    viewer: &Scope,
    action: &EvidenceFixAction,
    old_text: &str,
    new_text: &str,
    target_range: &str,
    apply: bool,
) -> (
    EvidenceFixerApplyActionStatus,
    String,
    Option<EvidenceFixerTombstone>,
) {
    let Some(page_id) = parse_page_subject(action) else {
        return blocked("missing or invalid page subject");
    };
    let Some(page) = eng.store.pages.get(&page_id) else {
        return skipped("page no longer exists");
    };
    if !document_visible_to_viewer(&page.scope, viewer) {
        return blocked("page is outside viewer scope");
    }
    let Some(next_markdown) = replace_line_range(&page.markdown, target_range, old_text, new_text)
    else {
        return blocked("semantic patch target range no longer matches old text");
    };
    let tombstone = page_tombstone(action, page.clone());
    if apply {
        let mut next = page.clone();
        next.markdown = next_markdown;
        next.updated_at = OffsetDateTime::now_utc();
        next.refresh_outbound_links();
        eng.write_page(next, "evidence-fixer");
        (
            EvidenceFixerApplyActionStatus::Applied,
            "applied semantic patch to target range".to_string(),
            Some(tombstone),
        )
    } else {
        (
            EvidenceFixerApplyActionStatus::WouldApply,
            "would apply semantic patch to target range".to_string(),
            Some(tombstone),
        )
    }
}

fn replace_line_range(
    markdown: &str,
    target_range: &str,
    old_text: &str,
    new_text: &str,
) -> Option<String> {
    let (start, end) = parse_line_range(target_range)?;
    let mut lines = markdown
        .lines()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if start == 0 || end < start || end > lines.len() {
        return None;
    }
    let slice = lines[start - 1..end].join("\n");
    if slice.trim() != old_text.trim() {
        return None;
    }
    let replacement = new_text
        .lines()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    lines.splice(start - 1..end, replacement);
    let mut out = lines.join("\n");
    if markdown.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

fn parse_line_range(target_range: &str) -> Option<(usize, usize)> {
    let (start, end) = target_range.split_once(':')?;
    Some((start.trim().parse().ok()?, end.trim().parse().ok()?))
}

fn has_web_evidence(action: &EvidenceFixAction) -> bool {
    action
        .evidence
        .iter()
        .any(|evidence| evidence.kind == "web_cross_verify")
}

fn parse_page_subject(action: &EvidenceFixAction) -> Option<PageId> {
    action
        .subject_id
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .map(PageId)
}

fn parse_claim_subject(action: &EvidenceFixAction) -> Option<ClaimId> {
    action
        .subject_id
        .as_deref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .map(ClaimId)
}

fn first_markdown_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let title = line
            .trim()
            .trim_start_matches('#')
            .trim_start()
            .trim()
            .to_string();
        (!title.is_empty()).then_some(title)
    })
}

fn page_tombstone(action: &EvidenceFixAction, page: wiki_core::WikiPage) -> EvidenceFixerTombstone {
    EvidenceFixerTombstone {
        tombstone_id: tombstone_id(action),
        action_id: action.action_id.clone(),
        subject_type: "page".into(),
        subject_id: page.id.0.to_string(),
        created_at: Some(OffsetDateTime::now_utc()),
        snapshot: EvidenceFixerSnapshot::Page { page },
    }
}

fn claim_tombstone(action: &EvidenceFixAction, claim: Claim) -> EvidenceFixerTombstone {
    EvidenceFixerTombstone {
        tombstone_id: tombstone_id(action),
        action_id: action.action_id.clone(),
        subject_type: "claim".into(),
        subject_id: claim.id.0.to_string(),
        created_at: Some(OffsetDateTime::now_utc()),
        snapshot: EvidenceFixerSnapshot::Claim { claim },
    }
}

fn tombstone_id(action: &EvidenceFixAction) -> String {
    format!("tombstone-{}-{}", action.action_id, Uuid::new_v4())
}

fn would_apply(
    reason: &str,
) -> (
    EvidenceFixerApplyActionStatus,
    String,
    Option<EvidenceFixerTombstone>,
) {
    (
        EvidenceFixerApplyActionStatus::WouldApply,
        reason.to_string(),
        None,
    )
}

fn applied(
    reason: &str,
) -> (
    EvidenceFixerApplyActionStatus,
    String,
    Option<EvidenceFixerTombstone>,
) {
    (
        EvidenceFixerApplyActionStatus::Applied,
        reason.to_string(),
        None,
    )
}

fn blocked(
    reason: &str,
) -> (
    EvidenceFixerApplyActionStatus,
    String,
    Option<EvidenceFixerTombstone>,
) {
    (
        EvidenceFixerApplyActionStatus::Blocked,
        reason.to_string(),
        None,
    )
}

fn skipped(
    reason: &str,
) -> (
    EvidenceFixerApplyActionStatus,
    String,
    Option<EvidenceFixerTombstone>,
) {
    (
        EvidenceFixerApplyActionStatus::Skipped,
        reason.to_string(),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{
        DomainSchema, EvidenceFixActionKind, EvidenceFixActionStatus, EvidenceFixEvidence,
        EvidenceFixPayload, EvidenceFixerPlan, MemoryTier, Scope, WikiPage,
    };

    fn private_scope() -> Scope {
        Scope::Private {
            agent_id: "cli".into(),
        }
    }

    fn plan_with(action: EvidenceFixAction) -> EvidenceFixerPlan {
        let mut plan = EvidenceFixerPlan {
            plan_id: "plan-1".into(),
            source_scan_report_id: "scan-1".into(),
            generated_at: None,
            viewer_scope: Some("private:cli".into()),
            summary: Default::default(),
            actions: vec![action],
        };
        plan.refresh_summary();
        plan
    }

    #[test]
    fn retire_page_apply_then_restore_round_trips_page() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let page = WikiPage::new("Delete me", "body", private_scope());
        let page_id = eng.write_page(page, "test");
        let plan = plan_with(EvidenceFixAction {
            action_id: "fix-1".into(),
            kind: EvidenceFixActionKind::RetirePage,
            status: EvidenceFixActionStatus::Ready,
            subject_type: "page".into(),
            subject_id: Some(page_id.0.to_string()),
            label: None,
            evidence: vec![],
            blockers: vec![],
            payload: EvidenceFixPayload::RetirePage {
                tombstone_required: true,
            },
        });

        let report = apply_evidence_fixer_plan(
            &mut eng,
            &private_scope(),
            &plan,
            EvidenceFixerApplyOptions {
                generated_at: OffsetDateTime::from_unix_timestamp(1).unwrap(),
                report_id: "apply-1".into(),
                policy: EvidenceFixerApplyPolicy::EvidenceAuto,
                apply: true,
            },
        );
        assert_eq!(report.summary.applied, 1);
        assert!(!eng.store.pages.contains_key(&page_id));

        let restore = restore_evidence_fixer_tombstone(
            &mut eng,
            &private_scope(),
            &report.tombstones[0],
            true,
            OffsetDateTime::from_unix_timestamp(2).unwrap(),
            "restore-1".into(),
        );
        assert_eq!(restore.status, EvidenceFixerApplyActionStatus::Applied);
        assert!(eng.store.pages.contains_key(&page_id));
    }

    #[test]
    fn merge_duplicate_claims_keeps_source_refs() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let s1 = eng.ingest_raw("file:///a", "a", private_scope(), "test");
        let s2 = eng.ingest_raw("file:///b", "b", private_scope(), "test");
        let c1 = eng.file_claim("same", private_scope(), MemoryTier::Semantic, "test");
        let c2 = eng.file_claim("same", private_scope(), MemoryTier::Semantic, "test");
        eng.attach_sources(c1, &[s1]).unwrap();
        eng.attach_sources(c2, &[s2]).unwrap();
        let plan = plan_with(EvidenceFixAction {
            action_id: "fix-1".into(),
            kind: EvidenceFixActionKind::MergeDuplicate,
            status: EvidenceFixActionStatus::Ready,
            subject_type: "duplicate_group".into(),
            subject_id: None,
            label: None,
            evidence: vec![EvidenceFixEvidence {
                kind: "duplicate".into(),
                detail: "exact".into(),
            }],
            blockers: vec![],
            payload: EvidenceFixPayload::MergeDuplicate {
                duplicate_kind: "claim_text".into(),
                member_ids: vec![c1.0.to_string(), c2.0.to_string()],
                confidence: "exact".into(),
            },
        });

        let report = apply_evidence_fixer_plan(
            &mut eng,
            &private_scope(),
            &plan,
            EvidenceFixerApplyOptions {
                generated_at: OffsetDateTime::from_unix_timestamp(1).unwrap(),
                report_id: "apply-1".into(),
                policy: EvidenceFixerApplyPolicy::EvidenceAuto,
                apply: true,
            },
        );

        assert_eq!(report.summary.applied, 1);
        assert!(!eng.store.claims.contains_key(&c2));
        let canonical = eng.store.claims.get(&c1).unwrap();
        assert_eq!(canonical.source_ids.len(), 2);
    }

    #[test]
    fn semantic_patch_only_replaces_target_range() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let page = WikiPage::new("Patch", "one\ntwo\nthree\n", private_scope());
        let page_id = eng.write_page(page, "test");
        let plan = plan_with(EvidenceFixAction {
            action_id: "fix-1".into(),
            kind: EvidenceFixActionKind::SemanticPatch,
            status: EvidenceFixActionStatus::Ready,
            subject_type: "page".into(),
            subject_id: Some(page_id.0.to_string()),
            label: None,
            evidence: vec![EvidenceFixEvidence {
                kind: "source_id".into(),
                detail: "s1".into(),
            }],
            blockers: vec![],
            payload: EvidenceFixPayload::SemanticPatch {
                old_text: "two".into(),
                new_text: "TWO".into(),
                target_range: "2:2".into(),
                source_ids: vec![],
            },
        });

        let report = apply_evidence_fixer_plan(
            &mut eng,
            &private_scope(),
            &plan,
            EvidenceFixerApplyOptions {
                generated_at: OffsetDateTime::from_unix_timestamp(1).unwrap(),
                report_id: "apply-1".into(),
                policy: EvidenceFixerApplyPolicy::EvidenceAuto,
                apply: true,
            },
        );
        assert_eq!(report.summary.applied, 1);
        assert_eq!(
            eng.store.pages.get(&page_id).unwrap().markdown,
            "one\nTWO\nthree\n"
        );
    }

    #[test]
    fn stale_plan_is_skipped_when_subject_is_missing() {
        let mut eng = LlmWikiEngine::new(DomainSchema::permissive_default());
        let plan = plan_with(EvidenceFixAction {
            action_id: "fix-1".into(),
            kind: EvidenceFixActionKind::RetirePage,
            status: EvidenceFixActionStatus::Ready,
            subject_type: "page".into(),
            subject_id: Some(Uuid::new_v4().to_string()),
            label: None,
            evidence: vec![],
            blockers: vec![],
            payload: EvidenceFixPayload::RetirePage {
                tombstone_required: true,
            },
        });

        let report = apply_evidence_fixer_plan(
            &mut eng,
            &private_scope(),
            &plan,
            EvidenceFixerApplyOptions {
                generated_at: OffsetDateTime::from_unix_timestamp(1).unwrap(),
                report_id: "apply-1".into(),
                policy: EvidenceFixerApplyPolicy::EvidenceAuto,
                apply: true,
            },
        );
        assert_eq!(report.summary.skipped, 1);
    }
}
