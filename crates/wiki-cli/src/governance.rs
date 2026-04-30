use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use wiki_core::GovernanceScanReport;

pub struct GovernanceScanFiles {
    pub json_path: PathBuf,
    pub markdown_path: PathBuf,
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiki_core::{GovernanceScanReport, GovernanceScanSummary, GovernanceSynthesisSignals};

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
}
