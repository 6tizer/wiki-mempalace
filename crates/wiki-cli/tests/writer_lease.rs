use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;
use wiki_storage::{SqliteRepository, SqliteWriterLease};

fn cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

#[test]
fn writer_command_fails_fast_when_lease_busy() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("wiki.db");
    let _repo = SqliteRepository::open(&db).unwrap();
    let _lease = SqliteWriterLease::acquire(&db, "holder", time::Duration::minutes(5)).unwrap();

    cli()
        .arg("--db")
        .arg(&db)
        .args(["ingest", "file:///a.md", "body"])
        .assert()
        .failure()
        .stderr(contains("writer lease busy"));
}

#[test]
fn read_only_metrics_does_not_require_writer_lease() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("wiki.db");
    let _repo = SqliteRepository::open(&db).unwrap();
    let _lease = SqliteWriterLease::acquire(&db, "holder", time::Duration::minutes(5)).unwrap();

    cli()
        .arg("--db")
        .arg(&db)
        .arg("metrics")
        .assert()
        .success()
        .stdout(contains("content:"));
}
