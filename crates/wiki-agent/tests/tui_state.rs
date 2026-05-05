use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn chat_tui_falls_back_to_cli_when_not_tty() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    std::fs::create_dir_all(db.parent().expect("db parent")).expect("db dir");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("chat")
        .arg("hello")
        .arg("--tui")
        .arg("--web")
        .arg("off")
        .arg("--fake-llm-response")
        .arg("ok")
        .assert()
        .success()
        .stderr(predicate::str::contains("falling back to CLI chat"))
        .stdout(predicate::str::contains("ok"));
}

#[test]
fn top_level_tui_falls_back_to_cli_when_not_tty() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    std::fs::create_dir_all(db.parent().expect("db parent")).expect("db dir");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("tui")
        .arg("hello")
        .arg("--web")
        .arg("off")
        .arg("--fake-llm-response")
        .arg("ok")
        .assert()
        .success()
        .stderr(predicate::str::contains("falling back to CLI chat"))
        .stdout(predicate::str::contains("ok"));
}
