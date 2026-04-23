use assert_cmd::Command;
use predicates::prelude::*;
use time::OffsetDateTime;
use wiki_core::WikiEvent;
use wiki_storage::{SqliteRepository, WikiRepository};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn append_outbox_query_events(repo: &SqliteRepository, count: i64) {
    for idx in 1..=count {
        repo.append_outbox(&WikiEvent::QueryServed {
            query_fingerprint: format!("q{idx}"),
            top_doc_ids: vec![format!("doc:{idx}")],
            at: OffsetDateTime::now_utc(),
        })
        .unwrap();
    }
}

#[test]
fn automation_run_daily_dry_run_prints_fixed_plan() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("run-daily")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation run-daily plan:"))
        .stdout(predicate::str::contains("1. batch-ingest"))
        .stdout(predicate::str::contains("2. lint"))
        .stdout(predicate::str::contains("3. maintenance"))
        .stdout(predicate::str::contains("4. consume-to-mempalace"))
        .stdout(predicate::str::contains("dry-run: no jobs executed"))
        .stdout(predicate::str::contains("automation: running").not());
}

#[test]
fn automation_status_prints_never_run_for_fresh_db() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation status:"))
        .stdout(predicate::str::contains("batch-ingest: never-run"))
        .stdout(predicate::str::contains("lint: never-run"))
        .stdout(predicate::str::contains("maintenance: never-run"))
        .stdout(predicate::str::contains("consume-to-mempalace: never-run"));
}

#[test]
fn automation_status_reads_latest_run_state() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    let lint_run = repo.start_automation_run("lint").unwrap();
    repo.mark_automation_run_succeeded(lint_run).unwrap();

    let maintenance_run = repo.start_automation_run("maintenance").unwrap();
    repo.mark_automation_run_failed(maintenance_run, "boom")
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("lint: status=succeeded"))
        .stdout(predicate::str::contains("maintenance: status=failed"))
        .stdout(predicate::str::contains("error_summary=boom"));
}

#[test]
fn automation_doctor_reports_empty_outbox_health() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation doctor:"))
        .stdout(predicate::str::contains(
            "outbox: head_id=0 total_events=0 unprocessed_events=0",
        ))
        .stdout(predicate::str::contains(
            "consumer mempalace: acked_up_to_id=never",
        ))
        .stdout(predicate::str::contains("backlog_events=0"));
}

#[test]
fn automation_doctor_reports_consumer_backlog() {
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
    repo.mark_outbox_processed(2, "mempalace").unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "outbox: head_id=3 total_events=3 unprocessed_events=1",
        ))
        .stdout(predicate::str::contains(
            "consumer mempalace: acked_up_to_id=2",
        ))
        .stdout(predicate::str::contains("backlog_events=1"));
}

#[test]
fn consume_to_mempalace_starts_from_latest_progress_and_advances_it() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    append_outbox_query_events(&repo, 3);
    repo.mark_outbox_processed(2, "mempalace").unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("consume-to-mempalace")
        .assert()
        .success()
        .stdout(predicate::str::contains("start_id=2"))
        .stdout(predicate::str::contains("acked=1"))
        .stdout(predicate::str::contains("consumer_tag=mempalace"));

    let progress_after = repo.get_outbox_consumer_progress("mempalace").unwrap();
    assert_eq!(progress_after.acked_up_to_id, Some(3));
    assert_eq!(progress_after.backlog_events, 0);
}

#[test]
fn consume_to_mempalace_empty_increment_does_not_ack() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    append_outbox_query_events(&repo, 2);
    repo.mark_outbox_processed(2, "archive").unwrap();

    let progress_before = repo.get_outbox_consumer_progress("archive").unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("consume-to-mempalace")
        .arg("--consumer-tag")
        .arg("archive")
        .assert()
        .success()
        .stdout(predicate::str::contains("consumed=0"))
        .stdout(predicate::str::contains("start_id=2"))
        .stdout(predicate::str::contains("acked=0"))
        .stdout(predicate::str::contains("consumer_tag=archive"));

    let progress_after = repo.get_outbox_consumer_progress("archive").unwrap();
    assert_eq!(progress_after, progress_before);
}
