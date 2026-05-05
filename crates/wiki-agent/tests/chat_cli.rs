use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
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

#[test]
fn chat_web_off_does_not_use_fake_web_evidence() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    let web = write_fake_web(&tmp, "Ignored Evidence");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("chat")
        .arg("latest test")
        .arg("--web")
        .arg("off")
        .arg("--web-evidence-json")
        .arg(&web)
        .arg("--fake-llm-response")
        .arg("answer")
        .assert()
        .success()
        .stdout(predicate::str::contains("web evidence:\n- off"))
        .stdout(predicate::str::contains("Ignored Evidence").not());
}

#[test]
fn chat_web_always_uses_fake_web_evidence_for_shared_scope() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    let web = write_fake_web(&tmp, "Cross Verified Evidence");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("--viewer-scope")
        .arg("shared:wiki")
        .arg("chat")
        .arg("latest test")
        .arg("--web")
        .arg("always")
        .arg("--web-evidence-json")
        .arg(&web)
        .arg("--fake-llm-response")
        .arg("answer")
        .assert()
        .success()
        .stdout(predicate::str::contains("web evidence:"))
        .stdout(predicate::str::contains("Cross Verified Evidence"))
        .stdout(predicate::str::contains("https://example.com/a"));
}

#[test]
fn chat_private_scope_blocks_web_by_default() {
    let tmp = tempdir().expect("tempdir");
    let db = tmp.path().join(".wiki").join("wiki.db");
    let web = write_fake_web(&tmp, "Private Should Not Leak");

    let mut cmd = Command::cargo_bin("wiki-agent").expect("wiki-agent binary");
    cmd.arg("--db")
        .arg(&db)
        .arg("chat")
        .arg("latest private question")
        .arg("--web")
        .arg("always")
        .arg("--web-evidence-json")
        .arg(&web)
        .arg("--fake-llm-response")
        .arg("answer")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "web evidence:\n- blocked: private scope",
        ))
        .stdout(predicate::str::contains("Private Should Not Leak").not());
}

fn write_fake_web(tmp: &tempfile::TempDir, title: &str) -> std::path::PathBuf {
    let path = tmp.path().join("web.json");
    let value = json!({
        "query": "latest test",
        "providers_requested": ["exa", "xai"],
        "providers_succeeded": ["exa", "xai"],
        "providers_failed": [],
        "distinct_domains": 2,
        "cross_verified": true,
        "evidence": [
            {
                "query": "latest test",
                "provider": "exa",
                "title": title,
                "url": "https://example.com/a",
                "domain": "example.com",
                "snippet": "snippet a",
                "summary": "summary a",
                "retrieved_at": "2026-05-05T00:00:00Z",
                "content_hash": "sha256:v1:a"
            },
            {
                "query": "latest test",
                "provider": "xai",
                "title": "Second source",
                "url": "https://example.org/b",
                "domain": "example.org",
                "snippet": "snippet b",
                "summary": "summary b",
                "retrieved_at": "2026-05-05T00:00:00Z",
                "content_hash": "sha256:v1:b"
            }
        ]
    });
    std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
    path
}
