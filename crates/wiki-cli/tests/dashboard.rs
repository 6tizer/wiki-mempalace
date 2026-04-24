use assert_cmd::Command;
use predicates::str::contains;
use std::path::Path;
use time::OffsetDateTime;
use wiki_core::WikiEvent;
use wiki_storage::{SqliteRepository, WikiRepository};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn append_query_event(db_path: &Path, fingerprint: &str) {
    let repo = SqliteRepository::open(db_path).unwrap();
    repo.append_outbox(&WikiEvent::QueryServed {
        query_fingerprint: fingerprint.to_string(),
        top_doc_ids: vec![format!("doc:{fingerprint}")],
        at: OffsetDateTime::now_utc(),
    })
    .unwrap();
}

#[test]
fn dashboard_writes_html_report() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let dashboard_path = temp_dir.path().join("reports").join("dashboard.html");

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("dashboard")
        .arg("--output")
        .arg(&dashboard_path)
        .assert()
        .success()
        .stdout(contains("dashboard_file="));

    let html = std::fs::read_to_string(dashboard_path).unwrap();
    assert!(html.contains("Automation Health"));
    assert!(html.contains("Metrics Summary"));
    assert!(html.contains("Outbox"));
    assert!(html.contains("Consumer"));
}

#[test]
fn dashboard_succeeds_without_existing_palace_db() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let dashboard_path = temp_dir.path().join("dashboard.html");
    let missing_palace = temp_dir.path().join("missing-palace.db");

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--palace")
        .arg(&missing_palace)
        .arg("dashboard")
        .arg("--output")
        .arg(&dashboard_path)
        .assert()
        .success()
        .stdout(contains("dashboard_file="));

    assert!(dashboard_path.exists());
    assert!(
        !missing_palace.exists(),
        "dashboard must not open or create palace db"
    );
}

#[test]
fn dashboard_is_read_only_except_output_file() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let wiki_dir = temp_dir.path().join("wiki");
    let dashboard_path = temp_dir.path().join("dashboard.html");

    append_query_event(&db_path, "readonly");
    let before = SqliteRepository::open(&db_path)
        .unwrap()
        .get_outbox_stats()
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("--sync-wiki")
        .arg("dashboard")
        .arg("--output")
        .arg(&dashboard_path)
        .assert()
        .success()
        .stdout(contains("dashboard_file="));

    let after = SqliteRepository::open(&db_path)
        .unwrap()
        .get_outbox_stats()
        .unwrap();
    assert_eq!(after, before);
    assert!(dashboard_path.exists());
    assert!(
        !wiki_dir.exists(),
        "dashboard must not write projection files"
    );
}

#[test]
fn dashboard_includes_custom_consumer_tag() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let dashboard_path = temp_dir.path().join("dashboard.html");

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("dashboard")
        .arg("--output")
        .arg(&dashboard_path)
        .arg("--consumer-tag")
        .arg("archive")
        .assert()
        .success()
        .stdout(contains("dashboard_file="));

    let html = std::fs::read_to_string(dashboard_path).unwrap();
    assert!(html.contains("archive"));
}

#[test]
fn dashboard_default_output_is_wiki_report_path() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();

    wiki_cli()
        .current_dir(temp_dir.path())
        .arg("--db")
        .arg(&db_path)
        .arg("dashboard")
        .assert()
        .success()
        .stdout(contains("dashboard_file=wiki/reports/dashboard.html"));

    let html =
        std::fs::read_to_string(temp_dir.path().join("wiki/reports/dashboard.html")).unwrap();
    assert!(html.contains("Wiki Dashboard"));
}
