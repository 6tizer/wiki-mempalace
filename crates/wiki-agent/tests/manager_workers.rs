use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;
use time::Duration;
use wiki_storage::SqliteWriterLease;

#[test]
fn agent_run_routes_lint_to_native_worker() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    std::fs::create_dir_all(db.parent().expect("db parent")).expect("create db dir");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("agent")
        .arg("run")
        .arg("lint the wiki")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"role_name\": \"lint_agent\""))
        .stdout(predicate::str::contains(
            "\"name\": \"collect_basic_lint_findings\"",
        ))
        .stdout(predicate::str::contains("\"status\": \"completed\""));
}

#[test]
fn agent_run_governance_uses_native_kernel_scan() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    std::fs::create_dir_all(db.parent().expect("db parent")).expect("create db dir");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("agent")
        .arg("run")
        .arg("governance audit")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"role_name\": \"governance_agent\"",
        ))
        .stdout(predicate::str::contains(
            "\"name\": \"run_governance_scan\"",
        ));
}

#[test]
fn agent_run_search_is_read_only_when_writer_lease_is_busy() {
    let tmp = tempdir().expect("tempdir");
    let db_dir = tmp.path().join(".wiki");
    std::fs::create_dir_all(&db_dir).expect("create db dir");
    let db = db_dir.join("wiki.db");
    let _lease = SqliteWriterLease::acquire(&db, "test-holder", Duration::minutes(5))
        .expect("hold writer lease");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("agent")
        .arg("run")
        .arg("search local wiki")
        .arg("--task")
        .arg("search")
        .arg("--web")
        .arg("off")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"role_name\": \"search_agent\""))
        .stdout(predicate::str::contains("\"status\": \"completed\""))
        .stdout(predicate::str::contains(
            "\"name\": \"query_ranked_with_ports\"",
        ));
}

#[test]
fn memory_curator_is_routed_and_blocks_without_sessions() {
    let tmp = tempdir().expect("tempdir");
    let db_dir = tmp.path().join(".wiki");
    std::fs::create_dir_all(&db_dir).expect("create db dir");
    let db = db_dir.join("wiki.db");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("agent")
        .arg("run")
        .arg("提炼技能")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"role_name\": \"memory_curator\"",
        ))
        .stdout(predicate::str::contains("\"status\": \"blocked\""))
        .stdout(predicate::str::contains("no sessions available"));
}

#[test]
fn agent_run_mcp_child_blocks_batch_worker_without_native_core() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    std::fs::create_dir_all(db.parent().expect("db parent")).expect("create db dir");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("agent")
        .arg("run")
        .arg("governance audit")
        .arg("--tool-backend")
        .arg("mcp-child")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"blocked\""))
        .stdout(predicate::str::contains(
            "requires native Rust core; mcp-child exposes only MCP tools",
        ));
}

#[test]
fn fixer_apply_is_blocked_when_writer_lease_is_busy() {
    let tmp = tempdir().expect("tempdir");
    let db_dir = tmp.path().join(".wiki");
    std::fs::create_dir_all(&db_dir).expect("create db dir");
    let db = db_dir.join("wiki.db");
    let _lease = SqliteWriterLease::acquire(&db, "test-holder", Duration::minutes(5))
        .expect("hold writer lease");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("agent")
        .arg("run")
        .arg("fix safe issues")
        .arg("--task")
        .arg("fixer")
        .arg("--apply")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"blocked\""))
        .stdout(predicate::str::contains("writer lease busy"));
}
