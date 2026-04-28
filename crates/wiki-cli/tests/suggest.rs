use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use wiki_core::{EntryType, Scope, WikiEvent, WikiPage};
use wiki_storage::{SqliteRepository, StorageSnapshot, WikiRepository};

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

fn seed_incomplete_concept_page(db_path: &Path) -> wiki_core::PageId {
    let scope = Scope::Private {
        agent_id: "cli".to_string(),
    };
    let page = WikiPage::new("Concept A", "## 定义\nOnly one section", scope)
        .with_entry_type(EntryType::Concept);
    let page_id = page.id;
    let repo = SqliteRepository::open(db_path).unwrap();
    repo.save_snapshot(&StorageSnapshot {
        pages: vec![page],
        ..StorageSnapshot::default()
    })
    .unwrap();
    page_id
}

fn find_executor_plan_json(report_dir: &Path) -> PathBuf {
    std::fs::read_dir(report_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with("executor-plan")
        })
        .expect("executor plan json")
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
fn suggest_executor_plan_json_wraps_report_and_plan() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--executor-plan")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let report_id = json["strategy_report"]["report_id"].as_str().unwrap();
    assert!(report_id.ends_with("-m12-suggest"));
    assert_eq!(
        json["strategy_report"]["suggestions"],
        serde_json::json!([])
    );
    assert_eq!(
        json["executor_plan"]["source_report_id"],
        serde_json::json!(report_id)
    );
    assert_eq!(json["executor_plan"]["mode"], "dry_run");
    assert_eq!(json["executor_plan"]["actions"], serde_json::json!([]));
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
fn suggest_executor_plan_report_dir_writes_plan_siblings() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let report_dir = temp_dir.path().join("suggestions");

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--executor-plan")
        .arg("--report-dir")
        .arg(&report_dir)
        .assert()
        .success()
        .stdout(contains("executor dry-run plan:"))
        .stdout(contains("executor_plan_json_file="))
        .stdout(contains("executor_plan_markdown_file="));

    let paths = std::fs::read_dir(&report_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    let json_files = paths
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .count();
    let markdown_files = paths
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .count();
    assert_eq!(json_files, 2, "{paths:?}");
    assert_eq!(markdown_files, 2, "{paths:?}");

    let plan_json = paths
        .iter()
        .find(|path| {
            path.file_stem()
                .unwrap()
                .to_string_lossy()
                .ends_with("executor-plan")
        })
        .expect("plan json or markdown file");
    let plan_id = plan_json.file_stem().unwrap().to_string_lossy();
    let plan_json_path = report_dir.join(format!("{plan_id}.json"));
    let plan_markdown_path = report_dir.join(format!("{plan_id}.md"));
    assert!(plan_json_path.exists(), "{paths:?}");
    assert!(plan_markdown_path.exists(), "{paths:?}");

    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(plan_json_path).unwrap()).unwrap();
    let markdown = std::fs::read_to_string(plan_markdown_path).unwrap();
    assert_eq!(json["plan_id"], serde_json::json!(plan_id.as_ref()));
    assert_eq!(json["mode"], "dry_run");
    assert!(markdown.contains("does not execute writes"));
}

#[test]
fn suggest_executor_apply_preflight_does_not_write() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let report_dir = temp_dir.path().join("suggestions");
    let page_id = seed_incomplete_concept_page(&db_path);

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--executor-plan")
        .arg("--report-dir")
        .arg(&report_dir)
        .assert()
        .success();
    let plan_path = find_executor_plan_json(&report_dir);
    let before = SqliteRepository::open(&db_path)
        .unwrap()
        .load_snapshot()
        .unwrap();
    let before_page = before
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .expect("seed page exists")
        .clone();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest-executor-apply")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--allow")
        .arg("fix-auto-safe")
        .assert()
        .success()
        .stdout(contains("mode=preflight"))
        .stdout(contains("would_apply=1"))
        .stdout(contains("applied=0"));

    let after = SqliteRepository::open(&db_path)
        .unwrap()
        .load_snapshot()
        .unwrap();
    let after_page = after
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .expect("page still exists");
    assert_eq!(after_page.markdown, before_page.markdown);
    assert_eq!(after_page.updated_at, before_page.updated_at);
}

#[test]
fn suggest_executor_apply_requires_explicit_allowlist() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let report_dir = temp_dir.path().join("suggestions");
    seed_incomplete_concept_page(&db_path);

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--executor-plan")
        .arg("--report-dir")
        .arg(&report_dir)
        .assert()
        .success();
    let plan_path = find_executor_plan_json(&report_dir);

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest-executor-apply")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--apply")
        .assert()
        .failure()
        .stderr(contains("--apply requires at least one --allow value"));
}

#[test]
fn suggest_executor_apply_runs_allowlisted_auto_fix() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let report_dir = temp_dir.path().join("suggestions");
    let apply_report_dir = temp_dir.path().join("apply-reports");
    let page_id = seed_incomplete_concept_page(&db_path);

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest")
        .arg("--executor-plan")
        .arg("--report-dir")
        .arg(&report_dir)
        .assert()
        .success();
    let plan_path = find_executor_plan_json(&report_dir);

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("suggest-executor-apply")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--allow")
        .arg("fix-auto-safe")
        .arg("--apply")
        .arg("--report-dir")
        .arg(&apply_report_dir)
        .assert()
        .success()
        .stdout(contains("mode=apply"))
        .stdout(contains("applied=1"))
        .stdout(contains("executor_apply_json_file="));

    let snapshot = SqliteRepository::open(&db_path)
        .unwrap()
        .load_snapshot()
        .unwrap();
    let page = snapshot
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .expect("page still exists");
    assert!(page.markdown.contains("待补充"), "{}", page.markdown);
    assert!(apply_report_dir.exists());
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
fn suggest_report_dir_without_value_uses_wiki_dir_reports() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let wiki_dir = temp_dir.path().join("vault");
    let report_dir = wiki_dir.join("reports").join("suggestions");

    wiki_cli()
        .current_dir(temp_dir.path())
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("suggest")
        .arg("--report-dir")
        .assert()
        .success()
        .stdout(contains(format!(
            "json_report_file={}/",
            report_dir.display()
        )))
        .stdout(contains(format!(
            "markdown_report_file={}/",
            report_dir.display()
        )));

    assert!(report_dir.exists());
    assert_eq!(
        std::fs::read_dir(&report_dir)
            .unwrap()
            .filter_map(Result::ok)
            .count(),
        2
    );
    assert!(
        !temp_dir.path().join("wiki/reports/suggestions").exists(),
        "suggest default must follow --wiki-dir, not cwd wiki/reports"
    );
}

#[test]
fn suggest_relative_report_dir_uses_wiki_dir() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let temp_dir = tempfile::tempdir().unwrap();
    let wiki_dir = temp_dir.path().join("vault");
    let report_dir = wiki_dir.join("reports").join("custom-suggestions");

    wiki_cli()
        .current_dir(temp_dir.path())
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("suggest")
        .arg("--report-dir")
        .arg("reports/custom-suggestions")
        .assert()
        .success()
        .stdout(contains(format!(
            "json_report_file={}/",
            report_dir.display()
        )));

    assert!(report_dir.exists());
    assert!(
        !temp_dir.path().join("reports/custom-suggestions").exists(),
        "relative suggest report dir must be vault-relative when --wiki-dir is set"
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
