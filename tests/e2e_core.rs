//! 端到端能力矩阵测试。
//!
//! 运行：`cargo test --test e2e_core`

use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rust-mempalace"))
}

fn unique_palace_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("rust_mempalace_e2e_{pid}_{nanos}_{seq}"))
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(path: PathBuf) -> Self {
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str], palace: &Path) -> std::process::Output {
    Command::new(bin())
        .env("NO_COLOR", "1")
        .args(["--palace"])
        .arg(palace)
        .args(args)
        .output()
        .expect("spawn cli")
}

fn run_mcp_once(request_line: &str, palace: &Path) -> std::process::Output {
    Command::new(bin())
        .env("NO_COLOR", "1")
        .args(["--palace"])
        .arg(palace)
        .args(["mcp", "--once", "--quiet"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin
                .as_mut()
                .expect("stdin")
                .write_all(request_line.as_bytes())?;
            c.wait_with_output()
        })
        .expect("spawn mcp")
}

fn mcp_roundtrip_lines(requests: &[Value], palace: &Path) -> Vec<Value> {
    let mut child = Command::new(bin())
        .env("NO_COLOR", "1")
        .args(["--palace"])
        .arg(palace)
        .args(["mcp", "--quiet"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn persistent mcp");

    {
        let stdin = child.stdin.as_mut().expect("mcp stdin");
        for req in requests {
            let line = serde_json::to_string(req).expect("serialize request");
            stdin.write_all(line.as_bytes()).expect("write request");
            stdin.write_all(b"\n").expect("write newline");
        }
    }

    let mut out = Vec::new();
    {
        let stdout = child.stdout.take().expect("mcp stdout");
        let mut reader = BufReader::new(stdout);
        for _ in requests {
            let mut line = String::new();
            let n = reader.read_line(&mut line).expect("read response line");
            assert!(n > 0, "mcp response line should not be empty");
            let json: Value = serde_json::from_str(line.trim()).expect("parse response json");
            out.push(json);
        }
    }

    let status = child.wait().expect("wait mcp");
    assert!(
        status.success(),
        "persistent mcp should exit success: {status:?}"
    );
    out
}

fn assert_ok(output: &std::process::Output, ctx: &str) {
    assert!(
        output.status.success(),
        "{ctx}: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn json_stdout(output: &std::process::Output) -> Value {
    let s = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(s.trim()).unwrap_or_else(|e| {
        panic!("invalid json stdout: {e}\nraw:\n{s}");
    })
}

fn setup_seeded_palace() -> (TempDir, PathBuf) {
    let palace = unique_palace_dir();
    let guard = TempDir::new(palace.clone());
    let fixtures = palace.join("fixtures");
    fs::create_dir_all(&fixtures).expect("fixtures dir");

    fs::write(
        fixtures.join("decisions.md"),
        r#"# Decisions
We decided to use Postgres for the primary database because we need strong consistency.
GraphQL was rejected for v1 due to scope.
"#,
    )
    .expect("write decisions");
    fs::write(
        fixtures.join("incidents.md"),
        r#"# Incident
Timeline: outage happened due to token expiration.
Retrospective suggests stricter token refresh tests.
"#,
    )
    .expect("write incidents");

    let out = run(
        &[
            "--quiet",
            "init",
            "--identity",
            "E2E identity: preserve verbatim decisions.",
        ],
        &palace,
    );
    assert_ok(&out, "init");

    let out = run(
        &[
            "--quiet",
            "--output",
            "json",
            "mine",
            fixtures.to_str().expect("fixtures path"),
        ],
        &palace,
    );
    assert_ok(&out, "mine");
    let filed = json_stdout(&out);
    assert!(
        filed
            .get("filed_drawers")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 0,
        "filed_drawers should be > 0: {filed}"
    );
    (guard, palace)
}

#[test]
fn e2e_cli_text_mode() {
    let (_guard, palace) = setup_seeded_palace();
    let out = run(
        &["--quiet", "search", "Postgres consistency", "--limit", "3"],
        &palace,
    );
    assert_ok(&out, "search text");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("score="),
        "text output should contain score: {text}"
    );
    assert!(text.contains("Postgres") || text.contains("decisions.md"));
}

#[test]
fn e2e_cli_json_mode() {
    let (_guard, palace) = setup_seeded_palace();
    let out = run(
        &[
            "--quiet",
            "--output",
            "json",
            "search",
            "Postgres database",
            "--limit",
            "5",
        ],
        &palace,
    );
    assert_ok(&out, "search json");
    let data = json_stdout(&out);
    let results = data
        .get("results")
        .and_then(|v| v.as_array())
        .expect("results array");
    assert!(!results.is_empty(), "json results should not be empty");
    let first = results[0].as_object().expect("first object");
    assert!(first.get("id").is_some());
    assert!(first.get("score").is_some());
    assert!(first.get("snippet").is_some());
}

#[test]
fn e2e_mcp_suite() {
    let (_guard, palace) = setup_seeded_palace();

    let list_req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    let out = run_mcp_once(list_req, &palace);
    assert_ok(&out, "mcp tools/list");
    let list_json = json_stdout(&out);
    let tools = list_json
        .get("result")
        .and_then(|v| v.get("tools"))
        .and_then(|v| v.as_array())
        .expect("tools array");
    assert!(
        tools
            .iter()
            .any(|t| t.get("name").and_then(|n| n.as_str()) == Some("mempalace_search"))
    );
    assert!(
        tools
            .iter()
            .any(|t| t.get("name").and_then(|n| n.as_str()) == Some("mempalace_kg_stats"))
    );

    let call_req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"mempalace_status","arguments":{}}}"#;
    let out = run_mcp_once(call_req, &palace);
    assert_ok(&out, "mcp tools/call status");
    let call_json = json_stdout(&out);
    assert!(
        call_json
            .get("result")
            .and_then(|r| r.get("drawers"))
            .is_some(),
        "mcp status should include drawers: {call_json}"
    );
}

#[test]
fn e2e_kg_conflict_timeline() {
    let (_guard, palace) = setup_seeded_palace();

    for obj in ["Orion", "Nova"] {
        let out = run(
            &[
                "--quiet",
                "kg-add",
                "--subject",
                "E2EUser",
                "--predicate",
                "works_on",
                "--object",
                obj,
            ],
            &palace,
        );
        assert_ok(&out, "kg-add");
    }

    let out = run(&["--quiet", "--output", "json", "kg-conflicts"], &palace);
    assert_ok(&out, "kg-conflicts");
    let conflicts = json_stdout(&out);
    let arr = conflicts
        .get("conflicts")
        .and_then(|v| v.as_array())
        .expect("conflicts array");
    assert!(!arr.is_empty(), "should detect conflicts");

    let out = run(
        &[
            "--quiet",
            "--output",
            "json",
            "kg-timeline",
            "--subject",
            "E2EUser",
        ],
        &palace,
    );
    assert_ok(&out, "kg-timeline");
    let timeline = json_stdout(&out);
    let events = timeline
        .get("timeline")
        .and_then(|v| v.as_array())
        .expect("timeline array");
    assert!(events.len() >= 2, "timeline should contain entries");
}

#[test]
fn e2e_bench_fixed_vs_random() {
    let (_guard, palace) = setup_seeded_palace();

    let out_fixed = run(
        &[
            "--quiet",
            "--output",
            "json",
            "bench",
            "--samples",
            "4",
            "--top-k",
            "2",
            "--mode",
            "fixed",
        ],
        &palace,
    );
    assert_ok(&out_fixed, "bench fixed");
    let fixed = json_stdout(&out_fixed);
    assert_eq!(fixed.get("mode").and_then(|v| v.as_str()), Some("fixed"));
    assert!(fixed.get("latency_ms").is_some());

    let out_random = run(
        &[
            "--quiet",
            "--output",
            "json",
            "bench",
            "--samples",
            "4",
            "--top-k",
            "2",
            "--mode",
            "random",
        ],
        &palace,
    );
    assert_ok(&out_random, "bench random");
    let random = json_stdout(&out_random);
    assert_eq!(random.get("mode").and_then(|v| v.as_str()), Some("random"));
    assert!(random.get("throughput_per_sec").is_some());
}

#[test]
fn e2e_agent_uses_mcp_toolchain() {
    let (_guard, palace) = setup_seeded_palace();

    let add = run(
        &[
            "--quiet",
            "kg-add",
            "--subject",
            "AgentUser",
            "--predicate",
            "prefers",
            "--object",
            "Postgres",
        ],
        &palace,
    );
    assert_ok(&add, "seed kg for agent");

    let requests = vec![
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"mempalace_search","arguments":{"query":"Postgres consistency","limit":3,"explain":true}}}),
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"mempalace_wake_up","arguments":{}}}),
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"mempalace_kg_query","arguments":{"subject":"AgentUser"}}}),
    ];
    let responses = mcp_roundtrip_lines(&requests, &palace);
    assert_eq!(responses.len(), 4);

    let tools = responses[0]
        .get("result")
        .and_then(|v| v.get("tools"))
        .and_then(|v| v.as_array())
        .expect("tools/list tools");
    assert!(
        tools
            .iter()
            .any(|t| t.get("name").and_then(|n| n.as_str()) == Some("mempalace_search")),
        "agent should discover mempalace_search"
    );

    let search_results = responses[1]
        .get("result")
        .and_then(|v| v.get("results"))
        .and_then(|v| v.as_array())
        .expect("search results");
    assert!(
        !search_results.is_empty(),
        "agent search should return hits"
    );
    assert!(
        search_results[0].get("score").is_some(),
        "search result should include score for agent rerank"
    );

    let wake_text = responses[2]
        .get("result")
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .expect("wake-up text");
    assert!(
        wake_text.contains("L0 Identity") && wake_text.contains("L1 Critical Facts"),
        "wake_up should provide layered memory context"
    );

    let facts = responses[3]
        .get("result")
        .and_then(|v| v.get("facts"))
        .and_then(|v| v.as_array())
        .expect("kg facts");
    assert!(
        facts
            .iter()
            .any(|f| f.get("object").and_then(|v| v.as_str()) == Some("Postgres")),
        "agent should retrieve KG preference fact"
    );
}

#[test]
fn e2e_agent_error_paths() {
    let (_guard, palace) = setup_seeded_palace();

    let requests = vec![
        // 未知工具
        serde_json::json!({"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"mempalace_unknown","arguments":{}}}),
        // 缺少必填 query
        serde_json::json!({"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"mempalace_search","arguments":{"limit":2}}}),
        // 非法 method
        serde_json::json!({"jsonrpc":"2.0","id":13,"method":"does/not/exist","params":{}}),
    ];
    let responses = mcp_roundtrip_lines(&requests, &palace);
    assert_eq!(responses.len(), 3);

    for (idx, resp) in responses.iter().enumerate() {
        let err = resp.get("error").and_then(|v| v.as_object());
        assert!(err.is_some(), "response[{idx}] should return error: {resp}");
        let code = err
            .and_then(|e| e.get("code"))
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        assert_eq!(
            code, -32000,
            "response[{idx}] should use unified server code"
        );
    }

    // 再测 parse error：发送非法 JSON，应返回 -32700
    let bad = run_mcp_once("{not-json}", &palace);
    assert_ok(&bad, "mcp parse error path process should still succeed");
    let parsed = json_stdout(&bad);
    let code = parsed
        .get("error")
        .and_then(|v| v.get("code"))
        .and_then(|v| v.as_i64())
        .expect("parse error code");
    assert_eq!(code, -32700, "invalid json should return parse error code");
}
