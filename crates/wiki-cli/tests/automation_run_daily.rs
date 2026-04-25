#![allow(clippy::too_many_arguments)]

use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::params;
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use wiki_core::WikiEvent;
use wiki_storage::{SqliteRepository, WikiRepository};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn make_minimal_vault(root: &Path) {
    std::fs::create_dir_all(root.join("pages/concept")).unwrap();
    std::fs::create_dir_all(root.join("sources/raw")).unwrap();
    std::fs::write(root.join("index.md"), "# Index\n").unwrap();
    std::fs::write(root.join("log.md"), "# Log\n").unwrap();
    std::fs::write(
        root.join("pages/concept/example.md"),
        concat!(
            "---\n",
            "id: page-1\n",
            "title: Example\n",
            "status: Draft\n",
            "---\n",
            "\n",
            "body\n"
        ),
    )
    .unwrap();
    std::fs::write(root.join("sources/raw/example.txt"), "source body\n").unwrap();
}

fn derive_backup_tar_path(db_backup: &Path) -> PathBuf {
    let parent = db_backup.parent().unwrap();
    let stem = db_backup.file_stem().unwrap().to_string_lossy();
    parent.join(format!("{stem}.tar.gz"))
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

fn seed_automation_run(
    db_path: &std::path::Path,
    job_name: &str,
    started_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
    status: &str,
    duration_ms: Option<i64>,
    error_summary: Option<&str>,
    heartbeat_at: OffsetDateTime,
) {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.execute(
        "INSERT INTO wiki_automation_run(job_name, started_at, finished_at, status, duration_ms, error_summary, heartbeat_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            job_name,
            started_at.format(&Rfc3339).unwrap(),
            finished_at.map(|value| value.format(&Rfc3339).unwrap()),
            status,
            duration_ms,
            error_summary,
            heartbeat_at.format(&Rfc3339).unwrap(),
        ],
    )
    .unwrap();
}

#[test]
fn automation_verify_restore_succeeds_for_valid_db_vault_and_palace() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();
    append_outbox_query_events(&repo, 2);

    let vault_dir = tempfile::tempdir().unwrap();
    make_minimal_vault(vault_dir.path());

    let palace_dir = tempfile::tempdir().unwrap();
    let palace_path = palace_dir.path().join("palace.db");
    let conn = rusqlite::Connection::open(&palace_path).unwrap();
    rust_mempalace::db::init_schema(&conn).unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(vault_dir.path())
        .arg("--palace")
        .arg(&palace_path)
        .arg("automation")
        .arg("verify-restore")
        .assert()
        .success()
        .stdout(predicate::str::contains("restore verify: status=ok"))
        .stdout(predicate::str::contains("wiki_db: integrity=ok"))
        .stdout(predicate::str::contains(
            "vault: index=ok log=ok pages=1 sources=1 frontmatter_checked=1",
        ))
        .stdout(predicate::str::contains("palace: status=ok"))
        .stdout(predicate::str::contains("consumer_tag=mempalace"));
}

#[test]
fn automation_verify_restore_fails_when_vault_missing_sources_dir() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();

    let vault_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(vault_dir.path().join("pages/concept")).unwrap();
    std::fs::write(vault_dir.path().join("index.md"), "# Index\n").unwrap();
    std::fs::write(vault_dir.path().join("log.md"), "# Log\n").unwrap();
    std::fs::write(
        vault_dir.path().join("pages/concept/example.md"),
        "---\nstatus: Draft\n---\nbody\n",
    )
    .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(vault_dir.path())
        .arg("automation")
        .arg("verify-restore")
        .assert()
        .failure()
        .stderr(predicate::str::contains("vault 缺少 sources/ 目录"));
}

#[test]
fn automation_verify_restore_fails_when_sources_dir_is_empty() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();

    let vault_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(vault_dir.path().join("pages/concept")).unwrap();
    std::fs::create_dir_all(vault_dir.path().join("sources")).unwrap();
    std::fs::write(vault_dir.path().join("index.md"), "# Index\n").unwrap();
    std::fs::write(vault_dir.path().join("log.md"), "# Log\n").unwrap();
    std::fs::write(
        vault_dir.path().join("pages/concept/example.md"),
        "---\nstatus: Draft\n---\nbody\n",
    )
    .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(vault_dir.path())
        .arg("automation")
        .arg("verify-restore")
        .assert()
        .failure()
        .stderr(predicate::str::contains("vault sources/ 下没有文件"));
}

#[test]
fn automation_verify_restore_fails_when_page_missing_status() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();

    let vault_dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(vault_dir.path().join("pages/concept")).unwrap();
    std::fs::create_dir_all(vault_dir.path().join("sources/raw")).unwrap();
    std::fs::write(vault_dir.path().join("index.md"), "# Index\n").unwrap();
    std::fs::write(vault_dir.path().join("log.md"), "# Log\n").unwrap();
    std::fs::write(
        vault_dir.path().join("pages/concept/example.md"),
        "---\nid: page-1\n---\nbody\n",
    )
    .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(vault_dir.path())
        .arg("automation")
        .arg("verify-restore")
        .assert()
        .failure()
        .stderr(predicate::str::contains("status field missing"));
}

#[test]
fn automation_verify_restore_fails_when_palace_missing_core_tables() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();

    let vault_dir = tempfile::tempdir().unwrap();
    make_minimal_vault(vault_dir.path());

    let palace_file = tempfile::NamedTempFile::new().unwrap();
    let palace_path = palace_file.path().to_owned();
    let conn = rusqlite::Connection::open(&palace_path).unwrap();
    conn.execute("CREATE TABLE hello(id INTEGER PRIMARY KEY)", [])
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(vault_dir.path())
        .arg("--palace")
        .arg(&palace_path)
        .arg("automation")
        .arg("verify-restore")
        .assert()
        .failure()
        .stderr(predicate::str::contains("palace.db 缺少核心表 drawers"));
}

#[test]
fn automation_verify_restore_fails_on_invalid_db_file() {
    let db = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(db.path(), "not a sqlite database").unwrap();

    let vault_dir = tempfile::tempdir().unwrap();
    make_minimal_vault(vault_dir.path());

    wiki_cli()
        .arg("--db")
        .arg(db.path())
        .arg("--wiki-dir")
        .arg(vault_dir.path())
        .arg("automation")
        .arg("verify-restore")
        .assert()
        .failure()
        .stderr(predicate::str::contains("database"));
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
fn automation_list_jobs_prints_registry() {
    wiki_cli()
        .arg("automation")
        .arg("list-jobs")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation jobs:"))
        .stdout(predicate::str::contains("batch-ingest daily=yes"))
        .stdout(predicate::str::contains("lint daily=yes"))
        .stdout(predicate::str::contains("maintenance daily=yes"))
        .stdout(predicate::str::contains("consume-to-mempalace daily=yes"))
        .stdout(predicate::str::contains("llm-smoke daily=no"));
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
        .stdout(predicate::str::contains("consume-to-mempalace: never-run"))
        .stdout(predicate::str::contains("llm-smoke: never-run"));
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
fn automation_run_lint_executes_only_target_job() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("run")
        .arg("lint")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation: running lint"))
        .stdout(predicate::str::contains("daily=yes"))
        .stdout(predicate::str::contains("status=succeeded"))
        .stdout(predicate::str::contains("duration_ms="));

    let repo = SqliteRepository::open(&db_path).unwrap();
    assert!(repo.get_latest_automation_run("lint").unwrap().is_some());
    assert!(repo
        .get_latest_automation_run("maintenance")
        .unwrap()
        .is_none());
    assert!(repo
        .get_latest_automation_run("consume-to-mempalace")
        .unwrap()
        .is_none());
    assert!(repo
        .get_latest_automation_run("llm-smoke")
        .unwrap()
        .is_none());
}

#[test]
fn automation_run_maintenance_executes_only_target_job() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("run")
        .arg("maintenance")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation: running maintenance"))
        .stdout(predicate::str::contains("status=succeeded"))
        .stdout(predicate::str::contains("duration_ms="));

    let repo = SqliteRepository::open(&db_path).unwrap();
    assert!(repo
        .get_latest_automation_run("maintenance")
        .unwrap()
        .is_some());
    assert!(repo.get_latest_automation_run("lint").unwrap().is_none());
    assert!(repo
        .get_latest_automation_run("consume-to-mempalace")
        .unwrap()
        .is_none());
}

#[test]
fn automation_run_unknown_job_fails() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("run")
        .arg("not-a-job")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"))
        .stderr(predicate::str::contains("not-a-job"));
}

#[test]
fn automation_last_failures_lists_recent_failures() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    let lint_run = repo.start_automation_run("lint").unwrap();
    repo.mark_automation_run_failed(lint_run, "lint boom")
        .unwrap();
    let maintenance_run = repo.start_automation_run("maintenance").unwrap();
    repo.mark_automation_run_failed(maintenance_run, "maintenance boom")
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("last-failures")
        .arg("--limit")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation last-failures:"))
        .stdout(predicate::str::contains("job=maintenance"))
        .stdout(predicate::str::contains("job=lint"))
        .stdout(predicate::str::contains("error_summary=maintenance boom"))
        .stdout(predicate::str::contains("error_summary=lint boom"));
}

#[test]
fn automation_health_red_on_stale_heartbeat_and_writes_summary() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();
    let summary = tempfile::NamedTempFile::new().unwrap();
    let summary_path = summary.path().to_owned();

    let now = OffsetDateTime::now_utc();
    seed_automation_run(
        &db_path,
        "batch-ingest",
        now - time::Duration::hours(37),
        None,
        "running",
        None,
        None,
        now - time::Duration::hours(36),
    );

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("health")
        .arg("--summary-file")
        .arg(&summary_path)
        .assert()
        .failure()
        .stdout(predicate::str::contains("automation health: status=red"))
        .stdout(predicate::str::contains("code=stale-heartbeat"))
        .stdout(predicate::str::contains("summary_file="))
        .stderr(predicate::str::contains("ALERT RED"));

    let summary_body = std::fs::read_to_string(&summary_path).unwrap();
    assert!(summary_body.contains("automation health: status=red"));
    assert!(summary_body.contains("code=stale-heartbeat"));
}

#[test]
fn automation_health_relative_summary_file_uses_wiki_dir() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();
    let temp_dir = tempfile::tempdir().unwrap();
    let wiki_dir = temp_dir.path().join("vault");
    let summary_path = wiki_dir.join("reports").join("automation-health.txt");

    wiki_cli()
        .current_dir(temp_dir.path())
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("automation")
        .arg("health")
        .arg("--summary-file")
        .arg("reports/automation-health.txt")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation health: status=green"))
        .stdout(predicate::str::contains(format!(
            "summary_file={}",
            summary_path.display()
        )));

    let summary_body = std::fs::read_to_string(&summary_path).unwrap();
    assert!(summary_body.contains("automation health: status=green"));
    assert!(
        !temp_dir
            .path()
            .join("reports/automation-health.txt")
            .exists(),
        "relative automation health summary must be vault-relative when --wiki-dir is set"
    );
}

#[test]
fn automation_health_red_on_consecutive_failures() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();

    let now = OffsetDateTime::now_utc();
    for idx in 0..3 {
        let started = now - time::Duration::hours(3 - idx);
        seed_automation_run(
            &db_path,
            "lint",
            started,
            Some(started + time::Duration::minutes(5)),
            "failed",
            Some(300_000),
            Some("lint timeout"),
            started + time::Duration::minutes(5),
        );
    }

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("health")
        .assert()
        .failure()
        .stdout(predicate::str::contains("code=consecutive-failures"))
        .stdout(predicate::str::contains("consecutive_failures=3"))
        .stderr(predicate::str::contains("ALERT RED"));
}

#[test]
fn automation_health_yellow_on_backlog_threshold() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    append_outbox_query_events(&repo, 30);
    repo.mark_outbox_processed(4, "mempalace").unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("health")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation health: status=yellow"))
        .stdout(predicate::str::contains("code=consumer-backlog"))
        .stdout(predicate::str::contains("backlog_events=26"))
        .stderr(predicate::str::contains("ALERT YELLOW"));
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
        .stdout(predicate::str::contains("seen=1"))
        .stdout(predicate::str::contains("dispatched=0"))
        .stdout(predicate::str::contains("ignored=1"))
        .stdout(predicate::str::contains("filtered=0"))
        .stdout(predicate::str::contains("unresolved=0"))
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
        .stdout(predicate::str::contains("seen=0"))
        .stdout(predicate::str::contains("dispatched=0"))
        .stdout(predicate::str::contains("ignored=0"))
        .stdout(predicate::str::contains("filtered=0"))
        .stdout(predicate::str::contains("unresolved=0"))
        .stdout(predicate::str::contains("start_id=2"))
        .stdout(predicate::str::contains("acked=0"))
        .stdout(predicate::str::contains("consumer_tag=archive"));

    let progress_after = repo.get_outbox_consumer_progress("archive").unwrap();
    assert_eq!(progress_after, progress_before);
}

#[test]
fn automation_run_consume_to_mempalace_executes_only_target_job() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    append_outbox_query_events(&repo, 2);

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("run")
        .arg("consume-to-mempalace")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "automation: running consume-to-mempalace",
        ))
        .stdout(predicate::str::contains("status=succeeded"))
        .stdout(predicate::str::contains("duration_ms="));

    assert!(repo
        .get_latest_automation_run("consume-to-mempalace")
        .unwrap()
        .is_some());
    assert!(repo.get_latest_automation_run("lint").unwrap().is_none());
    assert!(repo
        .get_latest_automation_run("maintenance")
        .unwrap()
        .is_none());
    assert!(repo
        .get_latest_automation_run("batch-ingest")
        .unwrap()
        .is_none());
}

#[test]
fn consume_to_mempalace_dispatches_active_events_from_cli_flow() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("ingest")
        .arg("file:///tmp/source.md")
        .arg("source body with redis")
        .arg("--scope")
        .arg("private:cli")
        .assert()
        .success();

    let claim_id_output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("file-claim")
        .arg("redis is enabled")
        .arg("--scope")
        .arg("private:cli")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let claim_id = String::from_utf8(claim_id_output).unwrap();
    let claim_id = claim_id
        .lines()
        .find_map(|line| line.strip_prefix("claim_id="))
        .unwrap()
        .trim()
        .to_string();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("supersede-claim")
        .arg(&claim_id)
        .arg("redis is enabled by default")
        .arg("--scope")
        .arg("private:cli")
        .assert()
        .success();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("consume-to-mempalace")
        .assert()
        .success()
        .stdout(predicate::str::contains("seen=3"))
        .stdout(predicate::str::contains("dispatched=3"))
        .stdout(predicate::str::contains("ignored=0"))
        .stdout(predicate::str::contains("filtered=0"))
        .stdout(predicate::str::contains("unresolved=0"))
        .stdout(predicate::str::contains("acked=3"));

    let progress_after = repo.get_outbox_consumer_progress("mempalace").unwrap();
    assert_eq!(progress_after.acked_up_to_id, Some(3));
}

#[test]
fn consume_to_mempalace_live_palace_uses_viewer_scope_bank_and_acks() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let palace = tempfile::NamedTempFile::new().unwrap();
    let palace_path = palace.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("file-claim")
        .arg("scope bank claim")
        .arg("--scope")
        .arg("private:cli")
        .assert()
        .success();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--viewer-scope")
        .arg("private:cli")
        .arg("--palace")
        .arg(&palace_path)
        .arg("consume-to-mempalace")
        .assert()
        .success()
        .stdout(predicate::str::contains("seen=1"))
        .stdout(predicate::str::contains("dispatched=1"))
        .stdout(predicate::str::contains("acked=1"))
        .stdout(predicate::str::contains("consumer_tag=mempalace"));

    let conn = rusqlite::Connection::open(&palace_path).unwrap();
    let cli_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM drawers WHERE bank_id = 'cli' AND content LIKE '%scope bank claim%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let wiki_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM drawers WHERE bank_id = 'wiki'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cli_rows, 1);
    assert_eq!(wiki_rows, 0);

    let progress_after = repo.get_outbox_consumer_progress("mempalace").unwrap();
    assert_eq!(progress_after.acked_up_to_id, Some(1));
    assert_eq!(progress_after.backlog_events, 0);
}

#[test]
fn consume_to_mempalace_ignores_query_crystallize_and_lint_events() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("file-claim")
        .arg("redis is enabled")
        .assert()
        .success();

    let seeded_head = repo.get_outbox_stats().unwrap().head_id;
    repo.mark_outbox_processed(seeded_head, "mempalace")
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("query")
        .arg("redis")
        .assert()
        .success();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("crystallize")
        .arg("What changed?")
        .arg("--finding")
        .arg("redis enabled")
        .assert()
        .success();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("lint")
        .assert()
        .success();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("consume-to-mempalace")
        .assert()
        .success()
        .stdout(predicate::str::contains("seen=3"))
        .stdout(predicate::str::contains("dispatched=0"))
        .stdout(predicate::str::contains("ignored=3"))
        .stdout(predicate::str::contains("filtered=0"))
        .stdout(predicate::str::contains("unresolved=0"))
        .stdout(predicate::str::contains("acked=3"));
}

#[test]
fn recovery_drill_script_restores_and_rebuilds_palace() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let vault_dir = tempfile::tempdir().unwrap();
    make_minimal_vault(vault_dir.path());

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("ingest")
        .arg("file:///tmp/source.md")
        .arg("source body with redis")
        .arg("--scope")
        .arg("private:cli")
        .assert()
        .success();

    let backup_out = tempfile::tempdir().unwrap();
    let backup_script = repo_root().join("scripts/backup.sh");
    let backup_output = std::process::Command::new("bash")
        .arg(&backup_script)
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki")
        .arg(vault_dir.path())
        .arg("--out")
        .arg(backup_out.path())
        .output()
        .unwrap();
    assert!(backup_output.status.success(), "{backup_output:?}");
    let backup_stdout = String::from_utf8(backup_output.stdout).unwrap();
    let backup_db = backup_stdout
        .lines()
        .find_map(|line| line.strip_prefix("BACKUP_DB="))
        .map(PathBuf::from)
        .unwrap();
    let backup_tar = derive_backup_tar_path(&backup_db);
    assert!(backup_tar.exists());

    let scratch = tempfile::tempdir().unwrap();
    let drill_script = repo_root().join("scripts/recovery-drill.sh");
    let output = std::process::Command::new("bash")
        .current_dir(repo_root())
        .arg(&drill_script)
        .arg("--db")
        .arg(&backup_db)
        .arg("--wiki-tar")
        .arg(&backup_tar)
        .arg("--scratch")
        .arg(scratch.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("RESTORED_DB="));
    assert!(stdout.contains("RESTORED_WIKI="));
    assert!(stdout.contains("RESTORED_PALACE="));
    assert!(stdout.contains("RECOVERY_DRILL_OK="));
    let restored_palace = stdout
        .lines()
        .find_map(|line| line.strip_prefix("RESTORED_PALACE="))
        .map(PathBuf::from)
        .unwrap();
    assert!(restored_palace.exists());
}

#[test]
fn recovery_drill_script_fails_when_sources_dir_missing_from_tar() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let _repo = SqliteRepository::open(&db_path).unwrap();

    let invalid_vault = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(invalid_vault.path().join("pages/concept")).unwrap();
    std::fs::write(invalid_vault.path().join("index.md"), "# Index\n").unwrap();
    std::fs::write(invalid_vault.path().join("log.md"), "# Log\n").unwrap();
    std::fs::write(
        invalid_vault.path().join("pages/concept/example.md"),
        "---\nstatus: Draft\n---\nbody\n",
    )
    .unwrap();

    let tar_path = tempfile::NamedTempFile::new().unwrap().into_temp_path();
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(tar_path.as_os_str())
        .arg("-C")
        .arg(invalid_vault.path().parent().unwrap())
        .arg(invalid_vault.path().file_name().unwrap())
        .status()
        .unwrap();
    assert!(status.success());

    let scratch = tempfile::tempdir().unwrap();
    let drill_script = repo_root().join("scripts/recovery-drill.sh");
    let output = std::process::Command::new("bash")
        .current_dir(repo_root())
        .arg(&drill_script)
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-tar")
        .arg(tar_path.as_os_str())
        .arg("--scratch")
        .arg(scratch.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("RECOVERY_DRILL_OK="));
}

#[test]
fn automation_run_batch_ingest_executes_only_target_job() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    let vault = tempfile::tempdir().unwrap();
    std::fs::create_dir(vault.path().join("sources")).unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(vault.path())
        .arg("automation")
        .arg("run")
        .arg("batch-ingest")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation: running batch-ingest"))
        .stdout(predicate::str::contains("status=succeeded"))
        .stdout(predicate::str::contains("duration_ms="));

    let repo = SqliteRepository::open(&db_path).unwrap();
    assert!(repo
        .get_latest_automation_run("batch-ingest")
        .unwrap()
        .is_some());
    assert!(repo.get_latest_automation_run("lint").unwrap().is_none());
    assert!(repo
        .get_latest_automation_run("maintenance")
        .unwrap()
        .is_none());
    assert!(repo
        .get_latest_automation_run("consume-to-mempalace")
        .unwrap()
        .is_none());
}

#[test]
fn automation_health_exit_on_yellow_flag_exits_nonzero_for_yellow() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let repo = SqliteRepository::open(&db_path).unwrap();

    // produce yellow backlog: 30 events, 4 acked → backlog=26, threshold yellow=25
    append_outbox_query_events(&repo, 30);
    repo.mark_outbox_processed(4, "mempalace").unwrap();

    // without --exit-on-yellow: yellow exits 0
    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("health")
        .assert()
        .success()
        .stdout(predicate::str::contains("automation health: status=yellow"))
        .stderr(predicate::str::contains("ALERT YELLOW"));

    // with --exit-on-yellow: yellow exits 1
    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("automation")
        .arg("health")
        .arg("--exit-on-yellow")
        .assert()
        .failure()
        .stdout(predicate::str::contains("automation health: status=yellow"))
        .stderr(predicate::str::contains("ALERT YELLOW"));
}
