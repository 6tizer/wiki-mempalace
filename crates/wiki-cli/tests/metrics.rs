use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use std::path::Path;
use time::OffsetDateTime;
use wiki_core::WikiEvent;
use wiki_storage::{SqliteRepository, WikiRepository};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn run_ingest(db_path: &Path, scope: &str, uri: &str, body: &str) {
    wiki_cli()
        .arg("--db")
        .arg(db_path)
        .arg("ingest")
        .arg(uri)
        .arg(body)
        .arg("--scope")
        .arg(scope)
        .assert()
        .success();
}

#[test]
fn metrics_empty_db_prints_text_groups() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("metrics")
        .assert()
        .success()
        .stdout(contains("metrics report:"))
        .stdout(contains("content:"))
        .stdout(contains("lint:"))
        .stdout(contains("gaps:"))
        .stdout(contains("outbox:"))
        .stdout(contains("lifecycle:"));
}

#[test]
fn metrics_json_is_parseable_and_contains_content() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("metrics")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json.get("content").is_some(), "{json:?}");
}

#[test]
fn metrics_report_writes_markdown_file() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let report_path = temp_dir.path().join("reports").join("metrics.md");

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("metrics")
        .arg("--report")
        .arg(&report_path)
        .assert()
        .success()
        .stdout(contains("report_file="));

    let markdown = std::fs::read_to_string(report_path).unwrap();
    assert!(markdown.contains("# Wiki Metrics Report"));
    assert!(markdown.contains("## Content"));
    assert!(markdown.contains("## Lint"));
    assert!(markdown.contains("## Gaps"));
    assert!(markdown.contains("## Outbox"));
    assert!(markdown.contains("## Lifecycle"));
}

#[test]
fn metrics_relative_report_uses_wiki_dir() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let wiki_dir = temp_dir.path().join("vault");
    let report_path = wiki_dir.join("reports").join("metrics.md");

    wiki_cli()
        .current_dir(temp_dir.path())
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("metrics")
        .arg("--report")
        .arg("reports/metrics.md")
        .assert()
        .success()
        .stdout(contains(format!("report_file={}", report_path.display())));

    let markdown = std::fs::read_to_string(&report_path).unwrap();
    assert!(markdown.contains("# Wiki Metrics Report"));
    assert!(
        !temp_dir.path().join("reports/metrics.md").exists(),
        "relative metrics report must be vault-relative when --wiki-dir is set"
    );
}

#[test]
fn metrics_respects_viewer_scope_for_content_counts() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    run_ingest(
        &db_path,
        "private:a",
        "file:///a.md",
        "alpha private source",
    );
    run_ingest(&db_path, "private:b", "file:///b.md", "beta private source");

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--viewer-scope")
        .arg("private:a")
        .arg("metrics")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["content"]["sources"], 1);
    assert_eq!(json["content"]["claims"], 0);
}

#[test]
fn metrics_does_not_write_outbox_or_projection_when_sync_wiki_is_set() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let wiki_dir = tempfile::tempdir().unwrap().path().join("wiki");

    run_ingest(
        &db_path,
        "private:cli",
        "file:///a.md",
        "alpha private source",
    );
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
        .arg("metrics")
        .assert()
        .success()
        .stdout(contains("metrics report:"));

    let after = SqliteRepository::open(&db_path)
        .unwrap()
        .get_outbox_stats()
        .unwrap();
    assert_eq!(after, before);
    assert!(
        !wiki_dir.exists(),
        "metrics must not write projection files"
    );
}

#[test]
fn metrics_custom_consumer_tag_reports_backlog() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();
    for idx in 1..=3 {
        repo.append_outbox(&WikiEvent::QueryServed {
            query_fingerprint: format!("q{idx}"),
            top_doc_ids: vec![format!("doc:{idx}")],
            at: OffsetDateTime::now_utc(),
        })
        .unwrap();
    }
    repo.mark_outbox_processed(1, "archive").unwrap();
    drop(repo);

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("metrics")
        .arg("--consumer-tag")
        .arg("archive")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["outbox"]["consumer_tag"], "archive");
    assert_eq!(json["outbox"]["acked_up_to_id"], 1);
    assert_eq!(json["outbox"]["backlog_events"], 2);
}
