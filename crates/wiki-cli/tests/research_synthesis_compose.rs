use assert_cmd::Command;
use serde_json::{json, Value};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn discovery_json() -> Value {
    json!({
        "report_id": "disc-1",
        "generated_at": null,
        "viewer_scope": "private:cli",
        "source_scan_report_id": "scan-1",
        "summary": {"total": 1, "single_tag": 0, "double_tag": 1, "triple_tag": 0, "quad_tag": 0},
        "candidates": [{
            "candidate_id": "synth-cand-0001",
            "kind": "double_tag",
            "tags": ["ai", "memory"],
            "anchor_tags": [],
            "concept_entity_pages": 3,
            "page_ids": [],
            "source_domains": ["example.com"],
            "score": 2010003,
            "rationale": "test"
        }]
    })
}

fn web_json() -> Value {
    json!([{
        "query": "AI memory validation",
        "providers_requested": ["exa", "tavily"],
        "providers_succeeded": ["exa", "tavily"],
        "providers_failed": [],
        "distinct_domains": 2,
        "cross_verified": true,
        "evidence": [
            {
                "query": "AI memory validation",
                "provider": "exa",
                "title": "Evidence A",
                "url": "https://a.example/evidence",
                "domain": "a.example",
                "snippet": "AI memory evidence A",
                "summary": "summary A",
                "retrieved_at": "2026-04-30T00:00:00Z",
                "content_hash": "sha256:v1:a"
            },
            {
                "query": "AI memory validation",
                "provider": "tavily",
                "title": "Evidence B",
                "url": "https://b.example/evidence",
                "domain": "b.example",
                "snippet": "AI memory evidence B",
                "summary": null,
                "retrieved_at": "2026-04-30T00:00:00Z",
                "content_hash": "sha256:v1:b"
            }
        ]
    }])
}

fn draft_json() -> Value {
    json!({
        "title": "AI Memory 综合研究",
        "research_question": "AI 和 memory 的连接是什么？",
        "comprehensive_analysis": "AI memory 的连接可以通过外部证据交叉验证。",
        "key_findings": [{
            "text": "AI memory 需要把内部知识和外部证据分开记录。",
            "citations": ["web:sha256:v1:a", "web:sha256:v1:b"]
        }],
        "action_recommendations": ["保留 evidence artifact。"]
    })
}

#[test]
fn synthesis_compose_with_fake_web_and_llm_applies_page() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("wiki.db");
    let discovery_path = temp.path().join("discovery.json");
    let web_path = temp.path().join("web.json");
    let draft_path = temp.path().join("draft.json");
    let verifier_path = temp.path().join("verifier.json");
    std::fs::write(
        &discovery_path,
        serde_json::to_string_pretty(&discovery_json()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        &web_path,
        serde_json::to_string_pretty(&web_json()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        &draft_path,
        serde_json::to_string_pretty(&draft_json()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        &verifier_path,
        serde_json::to_string_pretty(&json!({"approved": true, "blockers": []})).unwrap(),
    )
    .unwrap();

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("research-synthesis")
        .arg("compose")
        .arg("--candidate")
        .arg("synth-cand-0001")
        .arg("--discovery")
        .arg(&discovery_path)
        .arg("--web-evidence")
        .arg(&web_path)
        .arg("--draft-json")
        .arg(&draft_path)
        .arg("--verifier-json")
        .arg(&verifier_path)
        .arg("--allow-private-web-search")
        .arg("--apply")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "applied");
    assert_eq!(json["blockers"].as_array().unwrap().len(), 0);
    assert!(json["page_id"].as_str().is_some());
}

#[test]
fn synthesis_compose_blocks_private_web_without_override() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("wiki.db");
    let discovery_path = temp.path().join("discovery.json");
    std::fs::write(
        &discovery_path,
        serde_json::to_string_pretty(&discovery_json()).unwrap(),
    )
    .unwrap();

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("research-synthesis")
        .arg("compose")
        .arg("--candidate")
        .arg("synth-cand-0001")
        .arg("--discovery")
        .arg(&discovery_path)
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "blocked");
    assert!(json["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|blocker| blocker
            .as_str()
            .unwrap()
            .contains("private viewer scope requires")));
}
