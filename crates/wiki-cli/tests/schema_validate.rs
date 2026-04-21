//! schema-validate 子命令的集成测试：
//! - 合法 schema → exit 0 + stdout 含 "schema ok"
//! - 重复 EntryType → exit 1 + stderr 含关键字
//! - 自环晋升 → exit 1 + stderr 含关键字
//! - 环路晋升 → exit 1 + stderr 含关键字

use assert_cmd::Command;
use std::path::PathBuf;

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn fixture(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p.push(name);
    p
}

#[test]
fn schema_validate_valid_json() {
    wiki_cli()
        .arg("schema-validate")
        .arg(fixture("valid_schema.json"))
        .assert()
        .success()
        .stdout(predicates::str::contains("schema ok"))
        .stdout(predicates::str::contains("lifecycle_rules=1"));
}

#[test]
fn schema_validate_rejects_duplicate_entry_type() {
    wiki_cli()
        .arg("schema-validate")
        .arg(fixture("invalid_duplicate_entry_type.json"))
        .assert()
        .failure()
        .stderr(predicates::str::contains("重复"));
}

#[test]
fn schema_validate_rejects_self_loop() {
    wiki_cli()
        .arg("schema-validate")
        .arg(fixture("invalid_self_loop.json"))
        .assert()
        .failure()
        .stderr(predicates::str::contains("自环"));
}

#[test]
fn schema_validate_rejects_cycle() {
    wiki_cli()
        .arg("schema-validate")
        .arg(fixture("invalid_cycle.json"))
        .assert()
        .failure()
        .stderr(predicates::str::contains("环路"));
}
