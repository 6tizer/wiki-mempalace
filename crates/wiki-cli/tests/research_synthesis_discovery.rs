use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn scan_json() -> Value {
    json!({
        "report_id": "scan-1",
        "generated_at": null,
        "viewer_scope": "shared:wiki",
        "summary": {
            "lifecycle_total": 0,
            "lifecycle_promotable": 0,
            "lifecycle_blocked": 0,
            "references_total": 0,
            "lint_total": 0,
            "gaps_total": 0,
            "duplicate_groups_total": 0,
            "retire_candidates_total": 0,
            "tag_signal_total": 4,
            "tag_intersection_total": 4
        },
        "lifecycle": [],
        "references": [],
        "lint": [],
        "gaps": [],
        "duplicates": [],
        "retire_candidates": [],
        "synthesis_signals": {
            "tags": [
                {"tag": "a", "concept_entity_pages": 10, "page_ids": ["a1","a2","a3","a4","a5","a6","a7","a8","a9","a10"], "source_domains": ["a.example"]},
                {"tag": "b", "concept_entity_pages": 5, "page_ids": ["b1","b2","b3","b4","b5"], "source_domains": ["b.example"]},
                {"tag": "c", "concept_entity_pages": 5, "page_ids": ["c1","c2","c3","c4","c5"], "source_domains": ["c.example"]},
                {"tag": "d", "concept_entity_pages": 5, "page_ids": ["d1","d2","d3","d4","d5"], "source_domains": ["d.example"]}
            ],
            "intersections": [
                {"tags": ["a","b"], "concept_entity_pages": 2, "page_ids": ["ab1","ab2"]},
                {"tags": ["a","c"], "concept_entity_pages": 2, "page_ids": ["ac1","ac2"]},
                {"tags": ["b","c"], "concept_entity_pages": 2, "page_ids": ["bc1","bc2"]},
                {"tags": ["a","d"], "concept_entity_pages": 3, "page_ids": ["ad1","ad2","ad3"]}
            ],
            "deprecated_tags_used": [],
            "existing_topics": [
                {"page_id": "s-ab", "title": "ab", "tags": ["a","b"], "status": "in_review", "source_domains": []},
                {"page_id": "s-ac", "title": "ac", "tags": ["a","c"], "status": "in_review", "source_domains": []}
            ]
        }
    })
}

#[test]
fn synthesis_discovery_json_is_parseable_and_does_not_open_db() {
    let temp = tempfile::tempdir().unwrap();
    let scan_path = temp.path().join("scan.json");
    let db_path = temp.path().join("should-not-exist.db");
    std::fs::write(
        &scan_path,
        serde_json::to_string_pretty(&scan_json()).unwrap(),
    )
    .unwrap();

    let output = wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("research-synthesis")
        .arg("discover")
        .arg("--scan")
        .arg(&scan_path)
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["report_id"]
        .as_str()
        .unwrap()
        .ends_with("-synthesis-discovery"));
    assert_eq!(json["source_scan_report_id"], "scan-1");
    assert_eq!(json["summary"]["quad_tag"], 1);
    assert_eq!(json["summary"]["triple_tag"], 1);
    assert_eq!(json["candidates"][0]["kind"], "quad_tag");
    assert!(
        !db_path.exists(),
        "research-synthesis discover --scan must not create or open a wiki DB"
    );
}

#[test]
fn synthesis_discovery_report_dir_is_vault_relative() {
    let temp = tempfile::tempdir().unwrap();
    let wiki_dir = temp.path().join("vault");
    let report_dir = wiki_dir.join("reports").join("synthesis");
    let scan_path = temp.path().join("scan.json");
    std::fs::write(
        &scan_path,
        serde_json::to_string_pretty(&scan_json()).unwrap(),
    )
    .unwrap();

    wiki_cli()
        .current_dir(temp.path())
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("research-synthesis")
        .arg("discover")
        .arg("--scan")
        .arg(&scan_path)
        .arg("--report-dir")
        .arg("reports/synthesis")
        .assert()
        .success()
        .stdout(contains("synthesis discovery:"))
        .stdout(contains("json_report_file="))
        .stdout(contains("markdown_report_file="));

    let files: Vec<_> = std::fs::read_dir(&report_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert!(files.iter().any(|path| {
        path.extension().and_then(|ext| ext.to_str()) == Some("json")
            && path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .ends_with("synthesis-discovery")
    }));
    assert!(
        !temp.path().join("reports/synthesis").exists(),
        "relative synthesis report dir must be vault-relative when --wiki-dir is set"
    );
}
