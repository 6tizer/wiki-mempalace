use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use wiki_core::{
    EvidenceFixActionStatus, EvidenceFixerApplyReport, EvidenceFixerPlan,
    EvidenceFixerRestoreReport, GovernanceScanReport, SynthesisDiscoveryReport,
};

pub struct GovernanceScanFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
}

pub struct EvidenceFixerApplyFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
    pub tombstone_paths: Vec<PathBuf>,
}

pub fn governance_report_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-governance-scan",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub fn fixer_plan_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-evidence-fixer-plan",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub fn fixer_apply_report_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-evidence-fixer-apply",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub fn fixer_restore_report_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-evidence-fixer-restore",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub fn synthesis_discovery_report_prefix(generated_at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}.{:09}Z-synthesis-discovery",
        generated_at.year(),
        generated_at.month() as u8,
        generated_at.day(),
        generated_at.hour(),
        generated_at.minute(),
        generated_at.second(),
        generated_at.nanosecond()
    )
}

pub fn render_scan_text(report: &GovernanceScanReport) -> String {
    format!(
        concat!(
            "governance scan: report_id={} viewer_scope={} lifecycle={} promotable={} blocked={} ",
            "references={} lint={} gaps={} duplicates={} retire_candidates={} tag_signals={} tag_intersections={}\n"
        ),
        report.report_id,
        report.viewer_scope.as_deref().unwrap_or("none"),
        report.summary.lifecycle_total,
        report.summary.lifecycle_promotable,
        report.summary.lifecycle_blocked,
        report.summary.references_total,
        report.summary.lint_total,
        report.summary.gaps_total,
        report.summary.duplicate_groups_total,
        report.summary.retire_candidates_total,
        report.summary.tag_signal_total,
        report.summary.tag_intersection_total,
    )
}

pub fn render_fixer_plan_text(plan: &EvidenceFixerPlan) -> String {
    format!(
        concat!(
            "evidence fixer plan: plan_id={} scan={} viewer_scope={} total={} ready={} blocked={}\n"
        ),
        plan.plan_id,
        plan.source_scan_report_id,
        plan.viewer_scope.as_deref().unwrap_or("none"),
        plan.summary.total,
        plan.summary.ready,
        plan.summary.blocked,
    )
}

pub fn render_fixer_apply_text(report: &EvidenceFixerApplyReport) -> String {
    format!(
        concat!(
            "evidence fixer apply: report_id={} plan_id={} mode={:?} total={} ",
            "would_apply={} applied={} blocked={} skipped={} tombstones={}\n"
        ),
        report.report_id,
        report.plan_id,
        report.mode,
        report.summary.total,
        report.summary.would_apply,
        report.summary.applied,
        report.summary.blocked,
        report.summary.skipped,
        report.tombstones.len(),
    )
}

pub fn render_fixer_restore_text(report: &EvidenceFixerRestoreReport) -> String {
    format!(
        "evidence fixer restore: report_id={} tombstone_id={} mode={:?} status={:?} subject={}:{}\n",
        report.report_id,
        report.tombstone_id,
        report.mode,
        report.status,
        report.restored_subject_type,
        report.restored_subject_id,
    )
}

pub fn render_synthesis_discovery_text(report: &SynthesisDiscoveryReport) -> String {
    format!(
        concat!(
            "synthesis discovery: report_id={} scan={} viewer_scope={} total={} ",
            "single={} double={} triple={} quad={}\n"
        ),
        report.report_id,
        report.source_scan_report_id,
        report.viewer_scope.as_deref().unwrap_or("none"),
        report.summary.total,
        report.summary.single_tag,
        report.summary.double_tag,
        report.summary.triple_tag,
        report.summary.quad_tag,
    )
}

pub fn write_scan_files(
    report: &GovernanceScanReport,
    report_dir: &Path,
) -> Result<GovernanceScanFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let json_name = format!("{}.json", report.report_id);
    let markdown_name = format!("{}.md", report.report_id);
    let json_path = report_dir.join(&json_name);
    let markdown_path = report_dir.join(&markdown_name);
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    std::fs::write(&markdown_path, render_scan_markdown(report, &json_name))?;
    Ok(GovernanceScanFiles {
        json_path,
        markdown_path,
    })
}

pub fn write_fixer_plan_files(
    plan: &EvidenceFixerPlan,
    report_dir: &Path,
) -> Result<GovernanceScanFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let json_name = format!("{}.json", plan.plan_id);
    let markdown_name = format!("{}.md", plan.plan_id);
    let json_path = report_dir.join(&json_name);
    let markdown_path = report_dir.join(&markdown_name);
    std::fs::write(&json_path, serde_json::to_string_pretty(plan)?)?;
    std::fs::write(&markdown_path, render_fixer_plan_markdown(plan, &json_name))?;
    Ok(GovernanceScanFiles {
        json_path,
        markdown_path,
    })
}

pub fn write_fixer_apply_files(
    report: &EvidenceFixerApplyReport,
    report_dir: &Path,
) -> Result<EvidenceFixerApplyFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let json_name = format!("{}.json", report.report_id);
    let markdown_name = format!("{}.md", report.report_id);
    let json_path = report_dir.join(&json_name);
    let markdown_path = report_dir.join(&markdown_name);
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    std::fs::write(
        &markdown_path,
        render_fixer_apply_markdown(report, &json_name),
    )?;
    let mut tombstone_paths = Vec::new();
    for tombstone in &report.tombstones {
        let path = report_dir.join(format!("{}.json", tombstone.tombstone_id));
        std::fs::write(&path, serde_json::to_string_pretty(tombstone)?)?;
        tombstone_paths.push(path);
    }
    Ok(EvidenceFixerApplyFiles {
        json_path,
        markdown_path,
        tombstone_paths,
    })
}

pub fn write_fixer_restore_files(
    report: &EvidenceFixerRestoreReport,
    report_dir: &Path,
) -> Result<GovernanceScanFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let json_name = format!("{}.json", report.report_id);
    let markdown_name = format!("{}.md", report.report_id);
    let json_path = report_dir.join(&json_name);
    let markdown_path = report_dir.join(&markdown_name);
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    std::fs::write(
        &markdown_path,
        render_fixer_restore_markdown(report, &json_name),
    )?;
    Ok(GovernanceScanFiles {
        json_path,
        markdown_path,
    })
}

pub fn write_synthesis_discovery_files(
    report: &SynthesisDiscoveryReport,
    report_dir: &Path,
) -> Result<GovernanceScanFiles, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(report_dir)?;
    let json_name = format!("{}.json", report.report_id);
    let markdown_name = format!("{}.md", report.report_id);
    let json_path = report_dir.join(&json_name);
    let markdown_path = report_dir.join(&markdown_name);
    std::fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    std::fs::write(
        &markdown_path,
        render_synthesis_discovery_markdown(report, &json_name),
    )?;
    Ok(GovernanceScanFiles {
        json_path,
        markdown_path,
    })
}

pub fn render_scan_markdown(report: &GovernanceScanReport, sibling_json: &str) -> String {
    let mut out = format!(
        concat!(
            "# Governance Scan\n\n",
            "- report_id: `{}`\n",
            "- viewer_scope: `{}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth. This Markdown is rendered from the same scan report.\n\n",
            "## Summary\n\n",
            "- lifecycle: `{}`\n",
            "- promotable: `{}`\n",
            "- blocked: `{}`\n",
            "- references: `{}`\n",
            "- lint: `{}`\n",
            "- gaps: `{}`\n",
            "- duplicate groups: `{}`\n",
            "- retire candidates: `{}`\n",
            "- tag signals: `{}`\n",
            "- tag intersections: `{}`\n\n",
        ),
        report.report_id,
        report.viewer_scope.as_deref().unwrap_or("none"),
        sibling_json,
        sibling_json,
        report.summary.lifecycle_total,
        report.summary.lifecycle_promotable,
        report.summary.lifecycle_blocked,
        report.summary.references_total,
        report.summary.lint_total,
        report.summary.gaps_total,
        report.summary.duplicate_groups_total,
        report.summary.retire_candidates_total,
        report.summary.tag_signal_total,
        report.summary.tag_intersection_total,
    );

    out.push_str("## References\n\n");
    if report.references.is_empty() {
        out.push_str("No reference findings.\n\n");
    } else {
        for item in report.references.iter().take(50) {
            out.push_str(&format!(
                "- `{}` {} subject={} label={}\n",
                item.code,
                item.message,
                item.subject_id.as_deref().unwrap_or("none"),
                item.label.as_deref().unwrap_or("none"),
            ));
        }
        out.push('\n');
    }

    out.push_str("## Duplicates\n\n");
    if report.duplicates.is_empty() {
        out.push_str("No duplicate groups.\n\n");
    } else {
        for group in report.duplicates.iter().take(30) {
            out.push_str(&format!(
                "- `{}` confidence={} members={} key=`{}`\n",
                group.kind,
                group.confidence,
                group.members.len(),
                group.key
            ));
        }
        out.push('\n');
    }

    out.push_str("## Retire Candidates\n\n");
    if report.retire_candidates.is_empty() {
        out.push_str("No retire candidates.\n\n");
    } else {
        for item in &report.retire_candidates {
            out.push_str(&format!(
                "- `{}` {} reason={} evidence={}\n",
                item.page_id,
                item.title,
                item.reason,
                item.evidence.join(",")
            ));
        }
        out.push('\n');
    }

    out.push_str("## Synthesis Signals\n\n");
    if report.synthesis_signals.tags.is_empty() {
        out.push_str("No tag signals.\n");
    } else {
        for tag in report.synthesis_signals.tags.iter().take(50) {
            out.push_str(&format!(
                "- `{}` pages={} source_domains={}\n",
                tag.tag,
                tag.concept_entity_pages,
                if tag.source_domains.is_empty() {
                    "none".to_string()
                } else {
                    tag.source_domains.join(",")
                }
            ));
        }
    }
    out.push('\n');
    if !report.synthesis_signals.existing_topics.is_empty() {
        out.push_str("\nExisting synthesis topics:\n");
        for topic in report.synthesis_signals.existing_topics.iter().take(50) {
            out.push_str(&format!(
                "- `{}` tags=`{}` status={}\n",
                topic.title,
                topic.tags.join(","),
                topic
                    .status
                    .map(|status| format!("{status:?}"))
                    .unwrap_or_else(|| "none".to_string())
            ));
        }
    }
    out
}

pub fn render_fixer_plan_markdown(plan: &EvidenceFixerPlan, sibling_json: &str) -> String {
    let mut out = format!(
        concat!(
            "# Evidence Fixer Plan\n\n",
            "- plan_id: `{}`\n",
            "- source_scan_report_id: `{}`\n",
            "- viewer_scope: `{}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth. This Markdown is rendered from the same typed fixer plan.\n\n",
            "## Summary\n\n",
            "- total: `{}`\n",
            "- ready: `{}`\n",
            "- blocked: `{}`\n\n",
        ),
        plan.plan_id,
        plan.source_scan_report_id,
        plan.viewer_scope.as_deref().unwrap_or("none"),
        sibling_json,
        sibling_json,
        plan.summary.total,
        plan.summary.ready,
        plan.summary.blocked,
    );

    out.push_str("## Ready Actions\n\n");
    let mut ready_seen = false;
    for action in plan
        .actions
        .iter()
        .filter(|action| action.status == EvidenceFixActionStatus::Ready)
        .take(50)
    {
        ready_seen = true;
        out.push_str(&format!(
            "- `{}` {:?} subject={} label={}\n",
            action.action_id,
            action.kind,
            action.subject_id.as_deref().unwrap_or("none"),
            action.label.as_deref().unwrap_or("none"),
        ));
    }
    if !ready_seen {
        out.push_str("No ready actions.\n");
    }
    out.push('\n');

    out.push_str("## Blocked Actions\n\n");
    let mut blocked_seen = false;
    for action in plan
        .actions
        .iter()
        .filter(|action| action.status == EvidenceFixActionStatus::Blocked)
        .take(50)
    {
        blocked_seen = true;
        out.push_str(&format!(
            "- `{}` {:?} subject={} blockers={}\n",
            action.action_id,
            action.kind,
            action.subject_id.as_deref().unwrap_or("none"),
            if action.blockers.is_empty() {
                "none".to_string()
            } else {
                action.blockers.join(",")
            }
        ));
    }
    if !blocked_seen {
        out.push_str("No blocked actions.\n");
    }
    out
}

pub fn render_fixer_apply_markdown(
    report: &EvidenceFixerApplyReport,
    sibling_json: &str,
) -> String {
    let mut out = format!(
        concat!(
            "# Evidence Fixer Apply\n\n",
            "- report_id: `{}`\n",
            "- plan_id: `{}`\n",
            "- mode: `{:?}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth.\n\n",
            "## Summary\n\n",
            "- total: `{}`\n",
            "- would_apply: `{}`\n",
            "- applied: `{}`\n",
            "- blocked: `{}`\n",
            "- skipped: `{}`\n",
            "- tombstones: `{}`\n\n",
        ),
        report.report_id,
        report.plan_id,
        report.mode,
        sibling_json,
        sibling_json,
        report.summary.total,
        report.summary.would_apply,
        report.summary.applied,
        report.summary.blocked,
        report.summary.skipped,
        report.tombstones.len(),
    );
    out.push_str("## Actions\n\n");
    for action in report.actions.iter().take(80) {
        out.push_str(&format!(
            "- `{}` {:?} status={:?} reason={} tombstone={}\n",
            action.action_id,
            action.kind,
            action.status,
            action.reason,
            action.tombstone_id.as_deref().unwrap_or("none")
        ));
    }
    out
}

pub fn render_fixer_restore_markdown(
    report: &EvidenceFixerRestoreReport,
    sibling_json: &str,
) -> String {
    format!(
        concat!(
            "# Evidence Fixer Restore\n\n",
            "- report_id: `{}`\n",
            "- tombstone_id: `{}`\n",
            "- mode: `{:?}`\n",
            "- status: `{:?}`\n",
            "- subject: `{}:{}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth.\n"
        ),
        report.report_id,
        report.tombstone_id,
        report.mode,
        report.status,
        report.restored_subject_type,
        report.restored_subject_id,
        sibling_json,
        sibling_json,
    )
}

pub fn render_synthesis_discovery_markdown(
    report: &SynthesisDiscoveryReport,
    sibling_json: &str,
) -> String {
    let mut out = format!(
        concat!(
            "# Synthesis Discovery\n\n",
            "- report_id: `{}`\n",
            "- source_scan_report_id: `{}`\n",
            "- viewer_scope: `{}`\n",
            "- source_of_truth: `{}`\n\n",
            "> Sibling JSON `{}` is the source of truth. This Markdown is rendered from the same discovery report.\n\n",
            "## Summary\n\n",
            "- total: `{}`\n",
            "- single_tag: `{}`\n",
            "- double_tag: `{}`\n",
            "- triple_tag: `{}`\n",
            "- quad_tag: `{}`\n\n",
            "## Candidates\n\n",
        ),
        report.report_id,
        report.source_scan_report_id,
        report.viewer_scope.as_deref().unwrap_or("none"),
        sibling_json,
        sibling_json,
        report.summary.total,
        report.summary.single_tag,
        report.summary.double_tag,
        report.summary.triple_tag,
        report.summary.quad_tag,
    );
    if report.candidates.is_empty() {
        out.push_str("No candidates.\n");
    } else {
        for candidate in &report.candidates {
            out.push_str(&format!(
                "- `{}` {:?} tags=`{}` pages={} domains={} score={} reason={}\n",
                candidate.candidate_id,
                candidate.kind,
                candidate.tags.join(","),
                candidate.concept_entity_pages,
                if candidate.source_domains.is_empty() {
                    "none".to_string()
                } else {
                    candidate.source_domains.join(",")
                },
                candidate.score,
                candidate.rationale
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{
        EvidenceFixerPlan, GovernanceScanReport, GovernanceScanSummary, GovernanceSynthesisSignals,
    };

    #[test]
    fn render_scan_text_contains_key_counts() {
        let mut report = GovernanceScanReport {
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
        report.summary.references_total = 2;
        let text = render_scan_text(&report);
        assert!(text.contains("report_id=scan-1"));
        assert!(text.contains("references=2"));
    }

    #[test]
    fn render_fixer_plan_text_contains_key_counts() {
        let mut plan = EvidenceFixerPlan::new(
            "plan-1",
            "scan-1",
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            Some("shared:wiki".into()),
        );
        plan.refresh_summary();
        let text = render_fixer_plan_text(&plan);
        assert!(text.contains("plan_id=plan-1"));
        assert!(text.contains("ready=0"));
    }

    #[test]
    fn render_synthesis_discovery_text_contains_key_counts() {
        let mut report = SynthesisDiscoveryReport::new(
            "disc-1",
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            Some("shared:wiki".into()),
            "scan-1",
        );
        report.refresh_summary();
        let text = render_synthesis_discovery_text(&report);
        assert!(text.contains("report_id=disc-1"));
        assert!(text.contains("scan=scan-1"));
    }
}
