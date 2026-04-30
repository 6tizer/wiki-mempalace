use assert_cmd::Command;
use predicates::str::contains;
use wiki_core::{Claim, MemoryTier, RawArtifact, Scope};
use wiki_storage::{SqliteRepository, StorageSnapshot, WikiRepository};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn seed_row_state(db_path: &std::path::Path) {
    let repo = SqliteRepository::open(db_path).unwrap();
    let scope = Scope::Private {
        agent_id: "row-state-cli".into(),
    };
    let snapshot = StorageSnapshot {
        sources: vec![RawArtifact::new(
            "file:///row-state-cli.md",
            "row state body",
            scope.clone(),
        )],
        claims: vec![Claim::new("row state claim", scope, MemoryTier::Semantic)],
        ..StorageSnapshot::default()
    };
    repo.save_snapshot(&snapshot).unwrap();
}

#[test]
fn verify_row_state_succeeds_when_rows_match_blob() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    seed_row_state(&db_path);

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("verify-row-state")
        .assert()
        .success()
        .stdout(contains(
            "row_state status=ok rows=2 blob_present=true matches_blob=true",
        ))
        .stdout(contains("row_state_collection collection=claims rows=1"))
        .stdout(contains("row_state_collection collection=sources rows=1"));
}

#[test]
fn verify_row_state_json_has_no_startup_banner() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    seed_row_state(&db_path);

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("verify-row-state")
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["status"], "ok");
    assert_eq!(value["row_count"], 2);
    assert_eq!(value["matches_blob"], true);
}

#[test]
fn verify_row_state_fails_when_rows_are_absent() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    seed_row_state(&db_path);
    rusqlite::Connection::open(&db_path)
        .unwrap()
        .execute("DELETE FROM wiki_state_row", [])
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("verify-row-state")
        .assert()
        .failure()
        .stdout(contains(
            "row_state status=error rows=0 blob_present=true matches_blob=n/a",
        ))
        .stderr(contains("row-level state has no rows"));
}

#[test]
fn verify_row_state_reports_legacy_blob_only_db() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE wiki_state (
           id INTEGER PRIMARY KEY CHECK (id=1),
           payload_json TEXT NOT NULL
         );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO wiki_state (id, payload_json) VALUES (1, ?1)",
        [serde_json::to_string(&StorageSnapshot::default()).unwrap()],
    )
    .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("verify-row-state")
        .assert()
        .failure()
        .stdout(contains(
            "row_state status=error rows=0 blob_present=true matches_blob=n/a",
        ))
        .stderr(contains("row-level state has no rows"));
}
