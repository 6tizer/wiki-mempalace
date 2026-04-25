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

fn append_query_event(db_path: &Path, fingerprint: &str, top_doc_ids: Vec<String>) {
    let repo = SqliteRepository::open(db_path).unwrap();
    repo.append_outbox(&WikiEvent::QueryServed {
        query_fingerprint: fingerprint.to_string(),
        top_doc_ids,
        at: OffsetDateTime::now_utc(),
    })
    .unwrap();
}

#[test]
fn suggest_empty_db_prints_text_report() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .assert()
        .success()
        .stdout(contains("strategy suggestions:"))
        .stdout(contains("suggestions=0"));
}

#[test]
fn suggest_json_is_parseable() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["suggestions"], serde_json::json!([]));
    assert!(json["report_id"]
        .as_str()
        .unwrap()
        .ends_with("-m12-suggest"));
}

#[test]
fn suggest_report_dir_writes_json_and_markdown_siblings() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let report_dir = temp_dir.path().join("suggestions");

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--report-dir")
        .arg(&report_dir)
        .assert()
        .success()
        .stdout(contains("json_report_file="))
        .stdout(contains("markdown_report_file="));

    let mut json_files = Vec::new();
    let mut markdown_files = Vec::new();
    for entry in std::fs::read_dir(&report_dir).unwrap() {
        let path = entry.unwrap().path();
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => json_files.push(path),
            Some("md") => markdown_files.push(path),
            _ => {}
        }
    }
    assert_eq!(json_files.len(), 1, "{json_files:?}");
    assert_eq!(markdown_files.len(), 1, "{markdown_files:?}");

    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(&json_files[0]).unwrap()).unwrap();
    let markdown = std::fs::read_to_string(&markdown_files[0]).unwrap();
    let report_id = json["report_id"].as_str().unwrap();
    let json_name = json_files[0].file_name().unwrap().to_string_lossy();
    assert_eq!(json_name, format!("{report_id}.json"));
    assert_eq!(
        markdown_files[0].file_stem().unwrap().to_string_lossy(),
        report_id
    );
    assert!(markdown.contains(&format!("- report_id: {report_id}")));
    assert!(markdown.contains(&format!(
        "Sibling JSON `{json_name}` is the source of truth"
    )));
}

#[test]
fn suggest_report_dir_without_value_uses_default_directory() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();

    wiki_cli()
        .current_dir(temp_dir.path())
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--report-dir")
        .assert()
        .success()
        .stdout(contains("json_report_file=wiki/reports/suggestions/"))
        .stdout(contains("markdown_report_file=wiki/reports/suggestions/"));

    let report_dir = temp_dir.path().join("wiki/reports/suggestions");
    assert!(report_dir.exists());
    assert_eq!(
        std::fs::read_dir(&report_dir)
            .unwrap()
            .filter_map(Result::ok)
            .count(),
        2
    );
}

#[test]
fn suggest_report_files_preserve_history_when_run_twice() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let report_dir = temp_dir.path().join("suggestions");

    for _ in 0..2 {
        wiki_cli()
            .arg("--db")
            .arg(&db_path)
            .arg("suggest")
            .arg("--report-dir")
            .arg(&report_dir)
            .assert()
            .success();
    }

    let paths = std::fs::read_dir(&report_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    let json_count = paths
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count();
    let markdown_count = paths
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count();
    assert_eq!(json_count, 2, "{paths:?}");
    assert_eq!(markdown_count, 2, "{paths:?}");
}

#[test]
fn suggest_default_is_read_only_for_db_outbox_and_projection() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let wiki_dir = temp_dir.path().join("wiki");

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
        .arg("suggest")
        .assert()
        .success()
        .stdout(contains("strategy suggestions:"));

    let after = SqliteRepository::open(&db_path)
        .unwrap()
        .get_outbox_stats()
        .unwrap();
    assert_eq!(after, before);
    assert!(
        !wiki_dir.exists(),
        "suggest must not write projection files by default"
    );
}

#[test]
fn suggest_viewer_scope_does_not_leak_private_query_history() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    append_query_event(
        &db_path,
        "secret private query",
        vec!["page:00000000-0000-0000-0000-000000000001".to_string()],
    );
    append_query_event(
        &db_path,
        "secret private query",
        vec!["page:00000000-0000-0000-0000-000000000001".to_string()],
    );

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--viewer-scope")
        .arg("private:visible")
        .arg("suggest")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("secret private query"), "{stdout}");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let suggestions = json["suggestions"].as_array().unwrap();
    assert!(!suggestions
        .iter()
        .any(|item| item["code"] == "suggest.crystallize_candidate"));
}
