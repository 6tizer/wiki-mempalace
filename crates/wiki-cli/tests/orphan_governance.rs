#[path = "../src/orphan_governance.rs"]
mod orphan_governance;

use serde_json::Value;
use std::fs;
use tempfile::tempdir;

fn audit_json() -> &'static str {
    r#"{
  "vault_path": "/tmp/wiki",
  "generated_at": "2026-04-25T00:00:00Z",
  "orphan_candidates": {
    "total_files": 4,
    "samples_by_category": {
      "old_wiki_artifact": [".wiki/orphan-audit-report.md"],
      "unclassified_markdown": [
        "_archive/legacy-root/AGENTS md 5da673ca2377484498ec12f5679bfbf3.md",
        "_archive/legacy-root/README.md",
        "_archive/legacy-root/concepts/04ff4434.md"
      ]
    }
  },
  "readiness": {
    "unsupported_frontmatter": 12
  },
  "pages": {
    "missing_status": 5
  },
  "sources": {
    "compiled_to_wiki": {
      "true_count": 10,
      "false_count": 2,
      "missing": 16,
      "other": 0
    }
  }
}"#
}

#[test]
fn classifies_audit_counts_into_expected_lanes() {
    let temp = tempdir().unwrap();
    let audit_path = temp.path().join("vault-audit.json");
    fs::write(&audit_path, audit_json()).unwrap();

    let report = orphan_governance::build_report_from_audit_path(&audit_path).unwrap();

    assert_eq!(report.counts.orphan_candidates, 4);
    assert_eq!(report.counts.unsupported_frontmatter, 12);
    assert_eq!(report.counts.pages_missing_status, 5);
    assert_eq!(report.counts.sources_missing_compiled_to_wiki, 16);
    assert_eq!(report.lanes.len(), 4);
    assert_eq!(report.lanes[0].finding, "orphan_candidates");
    assert_eq!(report.lanes[0].lane, "human_required");
    assert_eq!(report.lanes[0].count, 4);
    assert_eq!(report.lanes[0].samples.len(), 4);
    assert_eq!(report.lanes[1].finding, "unsupported_frontmatter");
    assert_eq!(report.lanes[1].lane, "agent_review");
    assert_eq!(report.lanes[2].finding, "pages_missing_status");
    assert_eq!(report.lanes[2].lane, "future_auto_fix");
    assert_eq!(report.lanes[3].finding, "sources_missing_compiled_to_wiki");
    assert_eq!(report.lanes[3].lane, "agent_review");
    assert!(report.mutation_policy.writes_reports_only);
    assert!(!report.mutation_policy.vault_markdown_mutation);
    assert!(!report.mutation_policy.db_writes);
    assert!(!report.mutation_policy.outbox_emission);
    assert!(!report.mutation_policy.palace_writes);
    assert!(!report.mutation_policy.apply_mode);
}

#[test]
fn writes_json_and_markdown_reports_under_wiki_reports() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    let reports = vault.join("reports");
    fs::create_dir_all(&reports).unwrap();
    let audit_path = reports.join("vault-audit.json");
    fs::write(&audit_path, audit_json()).unwrap();
    let source_path = vault.join("sources/manual/source.md");
    fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    let source_body = "---\ntitle: Source\n---\nBody\n";
    fs::write(&source_path, source_body).unwrap();

    let before = fs::read_to_string(&source_path).unwrap();
    let (report, files) =
        orphan_governance::run_orphan_governance(&audit_path, None, Some(&vault)).unwrap();

    assert_eq!(
        files.json_path,
        reports.join("orphan-governance-report.json")
    );
    assert_eq!(
        files.markdown_path,
        reports.join("orphan-governance-report.md")
    );
    assert!(files.json_path.exists());
    assert!(files.markdown_path.exists());
    let json: Value = serde_json::from_str(&fs::read_to_string(&files.json_path).unwrap()).unwrap();
    assert_eq!(json["counts"]["orphan_candidates"], 4);
    assert_eq!(json["counts"]["unsupported_frontmatter"], 12);
    assert_eq!(json["counts"]["pages_missing_status"], 5);
    assert_eq!(json["counts"]["sources_missing_compiled_to_wiki"], 16);
    assert_eq!(json["lanes"][0]["lane"], "human_required");
    assert_eq!(json["mutation_policy"]["writes_reports_only"], true);
    let markdown = fs::read_to_string(&files.markdown_path).unwrap();
    assert!(markdown.contains("# Orphan Governance Report"));
    assert!(markdown.contains("source_of_truth: `orphan-governance-report.json`"));
    assert!(markdown.contains("- apply_mode: false"));
    assert_eq!(report.counts.orphan_candidates, 4);

    let after = fs::read_to_string(&source_path).unwrap();
    assert_eq!(before, after);
}

#[test]
fn rejects_report_dir_outside_wiki_reports_when_wiki_dir_is_set() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    fs::create_dir_all(vault.join("reports")).unwrap();
    let audit_path = vault.join("reports/vault-audit.json");
    fs::write(&audit_path, audit_json()).unwrap();

    let err = orphan_governance::run_orphan_governance(
        &audit_path,
        Some(temp.path().join("outside")),
        Some(&vault),
    )
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("orphan governance reports must be written under"));
}

#[test]
fn defaults_to_audit_parent_without_wiki_dir() {
    let temp = tempdir().unwrap();
    let audit_dir = temp.path().join("audit");
    fs::create_dir_all(&audit_dir).unwrap();
    let audit_path = audit_dir.join("vault-audit.json");
    fs::write(&audit_path, audit_json()).unwrap();

    let (_, files) = orphan_governance::run_orphan_governance(&audit_path, None, None).unwrap();

    assert_eq!(
        files.json_path,
        audit_dir.join("orphan-governance-report.json")
    );
    assert_eq!(
        files.markdown_path,
        audit_dir.join("orphan-governance-report.md")
    );
}

#[test]
fn rejects_malformed_audit_missing_required_fields() {
    let temp = tempdir().unwrap();
    let audit_path = temp.path().join("vault-audit.json");
    fs::write(&audit_path, "{}\n").unwrap();

    let err = orphan_governance::build_report_from_audit_path(&audit_path).unwrap_err();

    assert!(err
        .to_string()
        .contains("audit report missing required field: generated_at"));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_report_dir_escape_when_wiki_dir_is_set() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    let reports = vault.join("reports");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&reports).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let audit_path = reports.join("vault-audit.json");
    fs::write(&audit_path, audit_json()).unwrap();
    std::os::unix::fs::symlink(&outside, reports.join("escape")).unwrap();

    let err = orphan_governance::run_orphan_governance(
        &audit_path,
        Some(reports.join("escape")),
        Some(&vault),
    )
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("orphan governance reports must be written under"));
    assert!(!outside.join("orphan-governance-report.json").exists());
}
