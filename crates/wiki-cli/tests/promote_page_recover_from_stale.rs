//! D2 集成测试：needs_update → approved 反向 promotion 规则
//!
//! 流程（schema 无环设计：draft→needs_update→approved，无反向 approved→needs_update 避免 cycle）：
//! 1. crystallize 一个 entry_type=concept 的 page（初始 draft）
//! 2. promote-page --to needs_update --force（模拟 maintenance 把 draft 标为 stale）
//! 3. promote-page --to approved（不加 --force，验证 needs_update→approved 反向规则零条件通过）

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

/// 从 stdout 解析 "page=<uuid> claims=N" 中的 uuid（取第一个空格前的部分）
fn parse_page_id(stdout: &str) -> String {
    let line = stdout
        .lines()
        .find(|l| l.starts_with("page="))
        .expect("stdout 中应有 page=<id> 行");
    // 格式：page=<uuid> claims=N，取 = 之后到第一个空格/换行
    let after_eq = line.trim_start_matches("page=");
    after_eq
        .split_whitespace()
        .next()
        .expect("page= 后应有 uuid")
        .to_string()
}

#[test]
fn promote_page_can_recover_from_needs_update() {
    let db = tempfile::NamedTempFile::new().unwrap();
    let db_path = db.path().to_owned();
    let schema = fixture("schema_with_reverse_promotion.json");

    // Step 1：crystallize 生成 concept 页面
    let out = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--schema")
        .arg(&schema)
        .arg("crystallize")
        .arg("Test concept for reverse promotion")
        .arg("--entry-type")
        .arg("concept")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let page_id = parse_page_id(std::str::from_utf8(&out).unwrap());

    // Step 2：force-promote → needs_update（draft→needs_update，模拟 maintenance stale 标记）
    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--schema")
        .arg(&schema)
        .arg("promote-page")
        .arg(&page_id)
        .arg("--to")
        .arg("needs_update")
        .arg("--force")
        .assert()
        .success()
        .stdout(predicates::str::contains("promoted page"));

    // Step 3：反向规则 needs_update → approved（无 --force，验证规则零条件通过）
    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--schema")
        .arg(&schema)
        .arg("promote-page")
        .arg(&page_id)
        .arg("--to")
        .arg("approved")
        .assert()
        .success()
        .stdout(predicates::str::contains("promoted page"))
        .stdout(predicates::str::contains("Approved"));
}
