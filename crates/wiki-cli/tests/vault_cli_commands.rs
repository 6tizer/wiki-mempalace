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

fn orphan_audit_body(vault: &Path) -> String {
    serde_json::json!({
        "vault_path": vault.display().to_string(),
        "generated_at": "2026-04-25T00:00:00Z",
        "orphan_candidates": {
            "total_files": 0,
            "samples_by_category": {}
        },
        "readiness": {
            "unsupported_frontmatter": 0
        },
        "pages": {
            "missing_status": 0
        },
        "sources": {
            "compiled_to_wiki": {
                "true_count": 0,
                "false_count": 0,
                "missing": 1,
                "other": 0
            }
        },
        "path_lists": {
            "pages_missing_status": [],
            "sources_missing_compiled_to_wiki": ["sources/source-a.md"],
            "unsupported_frontmatter": [],
            "orphan_candidates": []
        }
    })
    .to_string()
}

fn orphan_plan_body(vault: &Path, audit_path: &Path) -> String {
    serde_json::json!({
        "version": 1,
        "generated_at": "2026-04-25T00:01:00Z",
        "audit_report_path": audit_path.display().to_string(),
        "vault_path": vault.display().to_string(),
        "audit_generated_at": "2026-04-25T00:00:00Z",
        "actions": [{
            "action_type": "insert_source_compiled_to_wiki",
            "path": "sources/source-a.md",
            "value": false,
            "confidence": 0.9,
            "reason": "CLI dry-run coverage.",
            "source": "rule"
        }],
        "markdown_report_model": {}
    })
    .to_string()
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

    let reports = vault.join("reports");
    let json_reports: Vec<_> = std::fs::read_dir(&reports)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("vault-audit-") && name.ends_with(".json"))
        })
        .collect();
    let markdown_reports: Vec<_> = std::fs::read_dir(&reports)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("vault-audit-") && name.ends_with(".md"))
        })
        .collect();
    assert_eq!(json_reports.len(), 1);
    assert_eq!(markdown_reports.len(), 1);
    assert!(!vault.join("reports/vault-audit.json").exists());
    assert!(!vault.join("reports/vault-audit.md").exists());
}

#[test]
fn orphan_governance_plan_cli_rejects_undated_audit_before_llm_config() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let audit_path = vault.join("reports/vault-audit.json");
    seed_vault(&vault);
    write_file(&audit_path, "{}");

    wiki_cmd()
        .arg("--wiki-dir")
        .arg(&vault)
        .arg("orphan-governance")
        .arg("plan")
        .arg("--audit-report")
        .arg(&audit_path)
        .assert()
        .failure()
        .stderr(contains(
            "requires timestamped vault-audit-<timestamp>.json",
        ));
}

#[test]
fn orphan_governance_apply_cli_defaults_to_dry_run() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    seed_vault(&vault);
    write_file(&audit_path, &orphan_audit_body(&vault));
    write_file(&plan_path, &orphan_plan_body(&vault, &audit_path));
    let before = std::fs::read_to_string(vault.join("sources/source-a.md")).unwrap();

    wiki_cmd()
        .arg("--wiki-dir")
        .arg(&vault)
        .arg("orphan-governance")
        .arg("apply")
        .arg("--plan")
        .arg(&plan_path)
        .assert()
        .success()
        .stdout(contains("orphan_governance_apply mode=dry_run"))
        .stdout(contains("source_compiled_to_wiki_insertions_planned=1"))
        .stdout(contains("source_compiled_to_wiki_insertions_applied=0"));

    let after = std::fs::read_to_string(vault.join("sources/source-a.md")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn sync_wiki_cli_does_not_create_root_concepts() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db = temp.path().join("wiki.db");
    seed_vault(&vault);

    wiki_cmd()
        .arg("--db")
        .arg(&db)
        .arg("--wiki-dir")
        .arg(&vault)
        .arg("--sync-wiki")
        .arg("ingest")
        .arg("file:///notes/a.md")
        .arg("Redis caching for API")
        .arg("--scope")
        .arg("shared:wiki")
        .assert()
        .success();

    assert!(
        !vault.join("concepts").exists(),
        "--sync-wiki must not create root concepts/"
    );
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
