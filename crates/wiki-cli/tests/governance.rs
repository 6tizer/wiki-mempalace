use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use std::path::Path;
use wiki_storage::SqliteRepository;

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
fn governance_scan_json_is_parseable_and_read_only() {
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

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("--sync-wiki")
        .arg("governance")
        .arg("scan")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["report_id"]
        .as_str()
        .unwrap()
        .ends_with("-governance-scan"));
    assert_eq!(json["viewer_scope"], "private:cli");
    assert!(json["summary"]["references_total"].is_number());

    let after = SqliteRepository::open(&db_path)
        .unwrap()
        .get_outbox_stats()
        .unwrap();
    assert_eq!(after, before);
    assert!(
        !wiki_dir.exists(),
        "governance scan must not write projection files"
    );
}

#[test]
fn governance_scan_report_dir_writes_json_and_markdown_siblings() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let wiki_dir = temp_dir.path().join("vault");
    let report_dir = wiki_dir.join("reports").join("governance");

    wiki_cli()
        .current_dir(temp_dir.path())
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("governance")
        .arg("scan")
        .arg("--report-dir")
        .arg("reports/governance")
        .assert()
        .success()
        .stdout(contains("governance scan:"))
        .stdout(contains("json_report_file="))
        .stdout(contains("markdown_report_file="));

    let files: Vec<_> = std::fs::read_dir(&report_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert!(files.iter().any(|path| {
        path.extension().and_then(|ext| ext.to_str()) == Some("json")
            && path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .ends_with("governance-scan")
    }));
    assert!(files.iter().any(|path| {
        path.extension().and_then(|ext| ext.to_str()) == Some("md")
            && path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .ends_with("governance-scan")
    }));
    assert!(
        !temp_dir.path().join("reports/governance").exists(),
        "relative governance report dir must be vault-relative when --wiki-dir is set"
    );
}
