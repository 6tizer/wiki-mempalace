use assert_cmd::Command;
use predicates::str::contains;
use std::path::Path;

fn wiki_cmd() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn write_file(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

fn source_body() -> &'static str {
    r#"---
title: "Source A"
notion_uuid: "11111111-1111-1111-1111-111111111111"
compiled_to_wiki: true
---

Source body.
"#
}

fn summary_page_body() -> &'static str {
    r#"---
title: "Summary A"
notion_uuid: "22222222-2222-2222-2222-222222222222"
entry_type: summary
status: approved
---

# Summary A

palace searchable body
"#
}

fn seed_vault(vault: &Path) {
    write_file(&vault.join("sources/source-a.md"), source_body());
    write_file(
        &vault.join("pages/summary/summary-a.md"),
        summary_page_body(),
    );
}

fn orphan_audit_body() -> &'static str {
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
  "readiness": { "unsupported_frontmatter": 12 },
  "pages": { "missing_status": 5 },
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
fn vault_audit_cli_writes_reports_under_vault_reports() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);

    wiki_cmd()
        .args(["vault-audit", "--vault"])
        .arg(&vault)
        .assert()
        .success()
        .stdout(contains("vault_audit sources=1 pages=1"));

    assert!(vault.join("reports/vault-audit.json").exists());
    assert!(vault.join("reports/vault-audit.md").exists());
}

#[test]
fn orphan_governance_cli_writes_reports_under_vault_reports() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let audit_path = vault.join("reports/vault-audit.json");
    seed_vault(&vault);
    write_file(&audit_path, orphan_audit_body());
    let before = std::fs::read_to_string(vault.join("sources/source-a.md")).unwrap();

    wiki_cmd()
        .arg("--wiki-dir")
        .arg(&vault)
        .arg("orphan-governance")
        .arg("--audit-report")
        .arg(&audit_path)
        .assert()
        .success()
        .stdout(contains(
            "orphan_governance orphan_candidates=4 unsupported_frontmatter=12 pages_missing_status=5 sources_missing_compiled_to_wiki=16",
        ));

    assert!(vault.join("reports/orphan-governance-report.json").exists());
    assert!(vault.join("reports/orphan-governance-report.md").exists());
    let after = std::fs::read_to_string(vault.join("sources/source-a.md")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn vault_backfill_cli_defaults_to_dry_run_without_db_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db = temp.path().join("wiki.db");
    seed_vault(&vault);

    wiki_cmd()
        .arg("--db")
        .arg(&db)
        .args(["vault-backfill", "--vault"])
        .arg(&vault)
        .assert()
        .success()
        .stdout(contains("vault_backfill mode=dry_run"));

    assert!(!db.exists());
    assert!(vault.join("reports/vault-backfill-report.json").exists());
    assert!(!std::fs::read_to_string(vault.join("sources/source-a.md"))
        .unwrap()
        .contains("source_id"));
}

#[test]
fn vault_backfill_apply_then_palace_init_cli_round_trips() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db = temp.path().join("wiki.db");
    let palace = temp.path().join("palace.db");
    seed_vault(&vault);

    wiki_cmd()
        .arg("--db")
        .arg(&db)
        .args(["vault-backfill", "--vault"])
        .arg(&vault)
        .args(["--apply", "--scope", "shared:wiki"])
        .assert()
        .success()
        .stdout(contains("page_written_events=1"));

    wiki_cmd()
        .arg("--db")
        .arg(&db)
        .arg("--wiki-dir")
        .arg(&vault)
        .args(["--viewer-scope", "shared:wiki"])
        .arg("--palace")
        .arg(&palace)
        .arg("palace-init")
        .assert()
        .success()
        .stdout(contains(
            "validation query_ok=true explain_ok=true fusion_ok=true",
        ));

    assert!(palace.exists());
    assert!(vault.join("reports/palace-init-report.json").exists());
}
