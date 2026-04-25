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
