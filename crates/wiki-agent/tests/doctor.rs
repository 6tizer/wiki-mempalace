use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn doctor_native_reports_twenty_two_tools() {
    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("doctor")
        .arg("--tool-backend")
        .arg("native")
        .assert()
        .success()
        .stdout(predicate::str::contains("tool_backend=native"))
        .stdout(predicate::str::contains("tools=22"))
        .stdout(predicate::str::contains("wiki_query"))
        .stdout(predicate::str::contains("mempalace_search"));
}
