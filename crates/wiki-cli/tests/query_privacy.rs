use assert_cmd::Command;
use wiki_core::{Scope, WikiEvent};
use wiki_storage::{SqliteRepository, WikiRepository};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

#[test]
fn query_outbox_hashes_query_and_records_viewer_scope() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let raw_query = "secret customer query";

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--viewer-scope")
        .arg("private:cli")
        .arg("query")
        .arg(raw_query)
        .assert()
        .success();

    let repo = SqliteRepository::open(&db_path).unwrap();
    let ndjson = repo.export_outbox_ndjson().unwrap();
    assert!(!ndjson.contains(raw_query));
    let event = ndjson
        .lines()
        .map(|line| serde_json::from_str::<WikiEvent>(line).unwrap())
        .find(|event| matches!(event, WikiEvent::QueryServed { .. }))
        .expect("query served event");

    let WikiEvent::QueryServed {
        query_fingerprint,
        query_hash: Some(query_hash),
        query_hash_schema_version: Some(1),
        schema_version: 2,
        viewer_scope: Some(Scope::Private { agent_id }),
        redacted_preview: None,
        ..
    } = event
    else {
        panic!("unexpected query event shape");
    };
    assert_eq!(agent_id, "cli");
    assert_eq!(query_fingerprint, query_hash);
    assert!(query_hash.starts_with("sha256:v1:"));
}
