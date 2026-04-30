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
            "references_total": 1,
            "lint_total": 1,
            "gaps_total": 0,
            "duplicate_groups_total": 1,
            "retire_candidates_total": 0,
            "tag_signal_total": 0,
            "tag_intersection_total": 0
        },
        "lifecycle": [],
        "references": [{
            "code": "reference.duplicate_source_id",
            "message": "duplicate source ref",
            "severity": "info",
            "subject_type": "claim",
            "subject_id": "c1",
            "label": null
        }],
        "lint": [{
            "code": "page.incomplete",
            "message": "页面缺少必需段落：来源引用",
            "severity": "warn",
            "subject": "p1"
        }],
        "gaps": [],
        "duplicates": [{
            "kind": "claim_text",
            "key": "same text",
            "confidence": "near",
            "members": [
                {"subject_type": "claim", "subject_id": "c1", "label": "Alpha"},
                {"subject_type": "claim", "subject_id": "c2", "label": "Alpha variant"}
            ]
        }],
        "retire_candidates": [],
        "synthesis_signals": {
            "tags": [],
            "intersections": [],
            "deprecated_tags_used": ["old-tag"]
        }
    })
}

#[test]
fn fixer_plan_json_is_parseable_and_does_not_open_db() {
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
        .arg("governance")
        .arg("fixer-plan")
        .arg("--scan")
        .arg(&scan_path)
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["plan_id"]
        .as_str()
        .unwrap()
        .ends_with("-evidence-fixer-plan"));
    assert_eq!(json["source_scan_report_id"], "scan-1");
    assert_eq!(json["summary"]["ready"], 2);
    assert_eq!(json["summary"]["blocked"], 2);
    assert!(
        !db_path.exists(),
        "fixer-plan must not create or open a wiki DB"
    );
}

#[test]
fn fixer_plan_report_dir_is_vault_relative() {
    let temp = tempfile::tempdir().unwrap();
    let wiki_dir = temp.path().join("vault");
    let report_dir = wiki_dir.join("reports").join("fixer");
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
        .arg("governance")
        .arg("fixer-plan")
        .arg("--scan")
        .arg(&scan_path)
        .arg("--report-dir")
        .arg("reports/fixer")
        .assert()
        .success()
        .stdout(contains("evidence fixer plan:"))
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
                .ends_with("evidence-fixer-plan")
    }));
    assert!(
        !temp.path().join("reports/fixer").exists(),
        "relative fixer report dir must be vault-relative when --wiki-dir is set"
    );
}

#[test]
fn fixer_plan_rejects_invalid_semantic_patch_json() {
    let temp = tempfile::tempdir().unwrap();
    let scan_path = temp.path().join("scan.json");
    let patch_path = temp.path().join("semantic.json");
    std::fs::write(
        &scan_path,
        serde_json::to_string_pretty(&scan_json()).unwrap(),
    )
    .unwrap();
    std::fs::write(&patch_path, "{not-json").unwrap();

    wiki_cli()
        .arg("governance")
        .arg("fixer-plan")
        .arg("--scan")
        .arg(&scan_path)
        .arg("--semantic-patches")
        .arg(&patch_path)
        .assert()
        .failure();
}
