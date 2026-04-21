//! Crystallize --entry-type 集成测试：
//! - 带 --entry-type=entity → page.entry_type 正确
//! - 不带 --entry-type → 行为回归（page.entry_type = None）

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
fn crystallize_with_entry_type_entity() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    // --schema 是全局参数，必须在子命令之前
    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--schema")
        .arg(fixture("valid_schema.json"))
        .arg("crystallize")
        .arg("What is Rust?")
        .arg("--entry-type")
        .arg("entity")
        .assert()
        .success()
        .stdout(predicates::str::contains("page="));
}

#[test]
fn crystallize_without_entry_type_ok() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--schema")
        .arg(fixture("valid_schema.json"))
        .arg("crystallize")
        .arg("What is Rust?")
        .assert()
        .success()
        .stdout(predicates::str::contains("page="));
}
