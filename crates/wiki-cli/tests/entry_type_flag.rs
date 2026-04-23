//! 集成测试：验证 --entry-type flag 在 query --write-page 下的行为。
//!
//! 流程：
//! 1. 在临时目录创建 wiki.db
//! 2. ingest 一条 source（确保 query 有数据）
//! 3. file-claim（确保 query 有命中的 doc）
//! 4. query --write-page --entry-type concept → 生成 page 并绑定 EntryType::Concept
//! 5. lint → 因为 page 没有 Concept 必需段落，应产生 page.incomplete finding

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// 辅助：获取 wiki-cli binary 路径
fn wiki_cli() -> Command {
    let bin = std::env::var("CARGO_BIN_EXE_wiki-cli")
        .expect("CARGO_BIN_EXE_wiki-cli not set; run via cargo test");
    Command::new(bin)
}

/// 辅助：创建临时目录
fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wiki-cli-test-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn query_write_page_with_entry_type_concept_triggers_incomplete_lint() {
    let tmp = tmp_dir("entry-type-query");
    let db = tmp.join("test.db");

    // 1. ingest 一条 source
    let status = wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("ingest")
        .arg("test://entry-type-test")
        .arg("This is a test document about Rust programming language.")
        .arg("--scope")
        .arg("private:test")
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "ingest failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    // 2. file-claim
    let status = wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("file-claim")
        .arg("Rust is a systems programming language.")
        .arg("--scope")
        .arg("private:test")
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "file-claim failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    // 3. query --write-page --entry-type concept
    let status = wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("query")
        .arg("Rust programming")
        .arg("--write-page")
        .arg("--page-title")
        .arg("test-concept-page")
        .arg("--entry-type")
        .arg("concept")
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "query --write-page --entry-type failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    // 4. lint → 应有 page.incomplete finding
    let output = wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("lint")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "lint failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // PageContract 保证了 Concept 类型的骨架段落完整（定义、关键要点、本文语境、来源引用），
    // 所以 lint 不应再报 page.incomplete。验证 lint 通过且无 incomplete finding。
    assert!(
        !stdout.contains("page.incomplete"),
        "PageContract 应保证 Concept 骨架完整，不应有 page.incomplete finding。lint output: {stdout}"
    );
}

#[test]
fn entry_type_rejects_unknown_value() {
    let tmp = tmp_dir("entry-type-invalid");
    let db = tmp.join("test.db");

    // --entry-type 设为不存在的值，应报错退出
    let status = wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("query")
        .arg("test")
        .arg("--write-page")
        .arg("--entry-type")
        .arg("nonexistent_type")
        .output()
        .unwrap();

    assert!(
        !status.status.success(),
        "should have failed for unknown entry type"
    );

    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(
        stderr.contains("未知")
            || stderr.contains("unknown")
            || stderr.contains("Unknown")
            || stderr.contains("error"),
        "expected error message about unknown entry type, got: {stderr}"
    );
}

#[test]
fn query_without_entry_type_no_incomplete_lint() {
    let tmp = tmp_dir("no-entry-type");
    let db = tmp.join("test.db");

    // ingest + query 不带 --entry-type → page.entry_type = None → lint 不报 page.incomplete
    wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("ingest")
        .arg("test://no-entry-type")
        .arg("Another test document.")
        .arg("--scope")
        .arg("private:test")
        .output()
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("file-claim")
        .arg("A claim.")
        .arg("--scope")
        .arg("private:test")
        .output()
        .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("query")
        .arg("test")
        .arg("--write-page")
        .arg("--page-title")
        .arg("no-entry-page")
        .output()
        .unwrap();

    let output = wiki_cli()
        .arg("--db")
        .arg(&db)
        .arg("lint")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // 没有 --entry-type 的 page 不应产生 page.incomplete
    assert!(
        !stdout.contains("page.incomplete"),
        "unexpected page.incomplete finding for page without entry_type: {stdout}"
    );
}
