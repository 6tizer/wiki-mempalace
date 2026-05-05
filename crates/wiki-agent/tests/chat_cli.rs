use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn chat_one_shot_uses_fake_llm_and_persists_session() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");

    let mut chat = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    chat.arg("--db")
        .arg(&db)
        .arg("chat")
        .arg("hello wiki")
        .arg("--fake-llm-response")
        .arg("hello back")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello back"));

    let mut sessions = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    sessions
        .arg("--db")
        .arg(&db)
        .arg("session")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("sessions"))
        .stdout(predicate::str::contains("hello wiki"));
}

#[test]
fn chat_repl_slash_tools_lists_native_registry() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("chat")
        .arg("--fake-llm-response")
        .arg("unused")
        .write_stdin("/tools\n/exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("tools=22 backend=native"))
        .stdout(predicate::str::contains("wiki_query"))
        .stdout(predicate::str::contains("mempalace_search"));
}

#[test]
fn chat_repl_profile_switch_is_reported() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("chat")
        .arg("--fake-llm-response")
        .arg("unused")
        .write_stdin("/profile agent_worker_lint\n/exit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("profile=agent_worker_lint"));
}

#[test]
fn chat_unknown_session_is_rejected() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("chat")
        .arg("--session")
        .arg("missing")
        .arg("--fake-llm-response")
        .arg("unused")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown session: missing"));
}
