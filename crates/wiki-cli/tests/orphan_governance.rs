#[path = "../src/orphan_governance.rs"]
mod orphan_governance;

use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_file(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn seed_vault(vault: &Path) {
    write_file(
        &vault.join("pages/concept/page-a.md"),
        "---\ntitle: Page A\nentry_type: concept\n---\n\n# Page A\n",
    );
    write_file(
        &vault.join("sources/source-a.md"),
        "---\ntitle: Source A\nkind: article\n---\n\nSource body.\n",
    );
    write_file(
        &vault.join("pages/summary/summary-a.md"),
        "---\ntitle: Source A\nentry_type: summary\nstatus: approved\n---\n\n# Source A\n",
    );
    write_file(&vault.join(".wiki/orphan-audit-report.md"), "old report\n");
}

fn audit_json(vault: &Path) -> String {
    json!({
        "vault_path": vault.display().to_string(),
        "generated_at": "2026-04-25T00:00:00Z",
        "orphan_candidates": {
            "total_files": 1,
            "samples_by_category": {
                "old_wiki_artifact": [".wiki/orphan-audit-report.md"]
            }
        },
        "readiness": {
            "unsupported_frontmatter": 0
        },
        "pages": {
            "missing_status": 1
        },
        "sources": {
            "compiled_to_wiki": {
                "true_count": 0,
                "false_count": 0,
                "missing": 1,
                "other": 0
            }
        },
        "path_lists": {
            "pages_missing_status": ["pages/concept/page-a.md"],
            "sources_missing_compiled_to_wiki": ["sources/source-a.md"],
            "unsupported_frontmatter": [],
            "orphan_candidates": [".wiki/orphan-audit-report.md"]
        }
    })
    .to_string()
}

fn valid_plan_json(vault: &Path, audit_path: &Path) -> String {
    json!({
        "version": 1,
        "generated_at": "2026-04-25T00:01:00Z",
        "audit_report_path": audit_path.display().to_string(),
        "vault_path": vault.display().to_string(),
        "audit_generated_at": "2026-04-25T00:00:00Z",
        "actions": [
            {
                "action_type": "insert_page_status",
                "path": "pages/concept/page-a.md",
                "value": "draft",
                "confidence": 0.98,
                "reason": "Page audit shows missing status.",
                "source": "rule"
            },
            {
                "action_type": "insert_source_compiled_to_wiki",
                "path": "sources/source-a.md",
                "value": false,
                "confidence": 0.92,
                "reason": "Source audit shows missing compiled_to_wiki.",
                "source": "rule"
            },
            {
                "action_type": "delete_cleanup_path",
                "path": ".wiki/orphan-audit-report.md",
                "value": "omitted",
                "confidence": 1.0,
                "reason": "Fixed cleanup whitelist contains this old report.",
                "source": "rule"
            },
            {
                "action_type": "needs_human",
                "path": "pages/summary/summary-a.md",
                "confidence": 0.7,
                "reason": "Summary candidate needs human relation review.",
                "source": "llm"
            },
            {
                "action_type": "recommend_batch_ingest",
                "path": "sources/source-a.md",
                "confidence": 0.5,
                "reason": "Only a recommendation; apply must not execute batch-ingest.",
                "source": "llm"
            }
        ],
        "markdown_report_model": {
            "summary": "validated"
        }
    })
    .to_string()
}

#[test]
fn plan_writes_timestamped_json_and_llm_markdown_after_validation() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let reports = vault.join("reports");
    fs::create_dir_all(&reports).unwrap();
    let audit_path = reports.join("vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));

    let (_, files) = orphan_governance::build_plan_from_llm_outputs(
        &audit_path,
        None,
        Some(&vault),
        &valid_plan_json(&vault, &audit_path),
        "# 孤儿治理计划\n\n不会执行 batch-ingest。\n",
    )
    .unwrap();

    assert!(files
        .json_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("orphan-governance-plan-"));
    assert_eq!(
        files.json_path.extension().and_then(|ext| ext.to_str()),
        Some("json")
    );
    assert_eq!(
        files.markdown_path.extension().and_then(|ext| ext.to_str()),
        Some("md")
    );
    assert!(!reports.join("orphan-governance-report.json").exists());
    let markdown = fs::read_to_string(files.markdown_path).unwrap();
    assert!(markdown.contains("## 补 page status（1 项）"));
    assert!(markdown.contains("## 补 source compiled_to_wiki（1 项）"));
    assert!(markdown.contains("## 删除清理白名单（1 项）"));
    assert!(markdown.contains("本计划不执行 batch-ingest"));
    assert!(!markdown.contains("不会执行 batch-ingest。"));
}

#[test]
fn plan_rejects_undated_vault_audit_json() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let audit_path = vault.join("reports/vault-audit.json");
    write_file(&audit_path, &audit_json(&vault));

    let err = orphan_governance::build_plan_from_llm_outputs(
        &audit_path,
        None,
        Some(&vault),
        &valid_plan_json(&vault, &audit_path),
        "# 孤儿治理计划\n",
    )
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("requires timestamped vault-audit-<timestamp>.json"));
}

#[test]
fn plan_rejects_llm_path_injection() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));
    let injected = json!({
        "version": 1,
        "generated_at": "2026-04-25T00:01:00Z",
        "audit_report_path": audit_path.display().to_string(),
        "vault_path": vault.display().to_string(),
        "audit_generated_at": "2026-04-25T00:00:00Z",
        "actions": [{
            "action_type": "insert_page_status",
            "path": "pages/concept/not-in-audit.md",
            "value": "draft",
            "confidence": 0.9,
            "reason": "invented",
            "source": "llm"
        }],
        "markdown_report_model": {}
    })
    .to_string();

    let err = orphan_governance::build_plan_from_llm_outputs(
        &audit_path,
        None,
        Some(&vault),
        &injected,
        "# 孤儿治理计划\n",
    )
    .unwrap_err();

    assert!(err.to_string().contains("references path outside evidence"));
}

#[test]
fn plan_ignores_external_markdown_path_not_in_validated_plan() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));

    let (_, files) = orphan_governance::build_plan_from_llm_outputs(
        &audit_path,
        None,
        Some(&vault),
        &valid_plan_json(&vault, &audit_path),
        "# 孤儿治理计划\n\n请处理 pages/concept/not-in-plan.md。\n",
    )
    .unwrap();

    let markdown = fs::read_to_string(files.markdown_path).unwrap();
    assert!(!markdown.contains("pages/concept/not-in-plan.md"));
}

#[test]
fn plan_ignores_external_markdown_commands() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));

    let (_, files) = orphan_governance::build_plan_from_llm_outputs(
        &audit_path,
        None,
        Some(&vault),
        &valid_plan_json(&vault, &audit_path),
        "# 孤儿治理计划\n\nrm -rf pages/concept/page-a.md\n",
    )
    .unwrap();

    let markdown = fs::read_to_string(files.markdown_path).unwrap();
    assert!(!markdown.contains("rm -rf"));
}

#[test]
fn apply_defaults_to_dry_run_without_mutating_files() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));
    write_file(&plan_path, &valid_plan_json(&vault, &audit_path));

    let page_before = fs::read_to_string(vault.join("pages/concept/page-a.md")).unwrap();
    let source_before = fs::read_to_string(vault.join("sources/source-a.md")).unwrap();
    let report = orphan_governance::run_apply(&plan_path, &vault, false).unwrap();

    assert_eq!(report.mode, "dry_run");
    assert_eq!(report.page_status_insertions_planned, 1);
    assert_eq!(report.source_compiled_to_wiki_insertions_planned, 1);
    assert_eq!(report.cleanup_deletions_planned, 1);
    assert_eq!(report.page_status_insertions_applied, 0);
    assert_eq!(
        fs::read_to_string(vault.join("pages/concept/page-a.md")).unwrap(),
        page_before
    );
    assert_eq!(
        fs::read_to_string(vault.join("sources/source-a.md")).unwrap(),
        source_before
    );
    assert!(vault.join(".wiki/orphan-audit-report.md").exists());
}

#[test]
fn apply_inserts_only_missing_frontmatter_fields_and_deletes_cleanup_whitelist() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));
    write_file(&plan_path, &valid_plan_json(&vault, &audit_path));

    let report = orphan_governance::run_apply(&plan_path, &vault, true).unwrap();

    assert_eq!(report.mode, "apply");
    assert_eq!(report.page_status_insertions_applied, 1);
    assert_eq!(report.source_compiled_to_wiki_insertions_applied, 1);
    assert_eq!(report.cleanup_deletions_applied, 1);
    let page = fs::read_to_string(vault.join("pages/concept/page-a.md")).unwrap();
    let source = fs::read_to_string(vault.join("sources/source-a.md")).unwrap();
    assert!(page.contains("status: draft\n"));
    assert!(source.contains("compiled_to_wiki: false\n"));
    assert!(!vault.join(".wiki/orphan-audit-report.md").exists());
}

#[test]
fn apply_rejects_non_whitelist_delete_path() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));
    let plan = json!({
        "version": 1,
        "generated_at": "2026-04-25T00:01:00Z",
        "audit_report_path": audit_path.display().to_string(),
        "vault_path": vault.display().to_string(),
        "audit_generated_at": "2026-04-25T00:00:00Z",
        "actions": [{
            "action_type": "delete_cleanup_path",
            "path": "pages/concept/page-a.md",
            "confidence": 1.0,
            "reason": "bad delete",
            "source": "llm"
        }],
        "markdown_report_model": {}
    });
    write_file(&plan_path, &plan.to_string());

    let err = orphan_governance::run_apply(&plan_path, &vault, true).unwrap_err();

    assert!(err
        .to_string()
        .contains("delete_cleanup_path path not in cleanup whitelist"));
    assert!(vault.join("pages/concept/page-a.md").exists());
}

#[test]
fn apply_revalidates_plan_against_audit_evidence() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    write_file(
        &vault.join("pages/concept/not-in-audit.md"),
        "---\ntitle: Not In Audit\nentry_type: concept\n---\n\n# Not In Audit\n",
    );
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));
    let plan = json!({
        "version": 1,
        "generated_at": "2026-04-25T00:01:00Z",
        "audit_report_path": audit_path.display().to_string(),
        "vault_path": vault.display().to_string(),
        "audit_generated_at": "2026-04-25T00:00:00Z",
        "actions": [{
            "action_type": "insert_page_status",
            "path": "pages/concept/not-in-audit.md",
            "value": "draft",
            "confidence": 0.9,
            "reason": "forged plan",
            "source": "llm"
        }],
        "markdown_report_model": {}
    });
    write_file(&plan_path, &plan.to_string());

    let err = orphan_governance::run_apply(&plan_path, &vault, true).unwrap_err();

    assert!(err.to_string().contains("references path outside evidence"));
    let page = fs::read_to_string(vault.join("pages/concept/not-in-audit.md")).unwrap();
    assert!(!page.contains("status: draft"));
}

#[test]
fn apply_rejects_plan_with_audit_outside_current_reports_dir() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = temp
        .path()
        .join("rogue-reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));
    write_file(&plan_path, &valid_plan_json(&vault, &audit_path));

    let err = orphan_governance::run_apply(&plan_path, &vault, true).unwrap_err();

    assert!(err
        .to_string()
        .contains("audit report must be under current wiki reports directory"));
}

#[test]
fn apply_rejects_audit_vault_path_mismatch() {
    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    let other_vault = temp.path().join("other-vault");
    seed_vault(&vault);
    seed_vault(&other_vault);
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&other_vault));
    write_file(&plan_path, &valid_plan_json(&vault, &audit_path));

    let err = orphan_governance::run_apply(&plan_path, &vault, true).unwrap_err();

    assert!(err
        .to_string()
        .contains("audit report vault_path does not match current wiki_dir"));
}

#[cfg(unix)]
#[test]
fn apply_rejects_symlink_write_target() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().unwrap();
    let vault = temp.path().join("vault");
    seed_vault(&vault);
    let outside = temp.path().join("outside.md");
    write_file(
        &outside,
        "---\ntitle: Outside\nentry_type: concept\n---\n\n# Outside\n",
    );
    std::fs::remove_file(vault.join("pages/concept/page-a.md")).unwrap();
    symlink(&outside, vault.join("pages/concept/page-a.md")).unwrap();
    let plan_path = vault.join("reports/orphan-governance-plan-2026-04-25T000001Z.json");
    let audit_path = vault.join("reports/vault-audit-2026-04-25T000000Z.json");
    write_file(&audit_path, &audit_json(&vault));
    write_file(&plan_path, &valid_plan_json(&vault, &audit_path));

    let err = orphan_governance::run_apply(&plan_path, &vault, true).unwrap_err();

    assert!(err.to_string().contains("refuses symlink path"));
    let outside_body = fs::read_to_string(outside).unwrap();
    assert!(!outside_body.contains("status: draft"));
}
