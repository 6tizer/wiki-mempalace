#[path = "../src/consistency.rs"]
#[allow(dead_code)]
mod consistency;

use consistency::{
    find_source_summary_candidates, find_stale_notion_link_candidates, run_consistency_apply,
    run_consistency_plan, ConsistencyActionKind, ConsistencyPlan, ConsistencyPlanAction,
    DbPageEvidence, DbSourceEvidence, SourceSummaryCandidateKind,
};

#[test]
fn stale_notion_style_links_are_candidates_only() {
    let pages = vec![DbPageEvidence {
        id: "page-a".into(),
        title: "Page A".into(),
        markdown: "See [[AGENTS%20md]], [[Encoded%2EPage.md]] and [old](Legacy%20Export%20abcdef1234567890abcdef1234567890.md).\nAlso [normal](pages/summary/current.md) and [中文](../concept/混合检索.md).".into(),
        entry_type: Some("concept".into()),
    }];

    let candidates = find_stale_notion_link_candidates(&pages);

    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[0].page_id, "page-a");
    assert_eq!(candidates[0].raw_target, "AGENTS%20md");
    assert_eq!(candidates[0].decoded_target, "AGENTS md");
    assert_eq!(candidates[0].action, "candidate_only");
    assert!(candidates[0].reason.contains("url_encoded"));
    assert_eq!(candidates[1].raw_target, "Encoded%2EPage.md");
    assert_eq!(candidates[1].decoded_target, "Encoded.Page.md");
    assert!(candidates[1].reason.contains("url_encoded"));
    assert_eq!(
        candidates[2].decoded_target,
        "Legacy Export abcdef1234567890abcdef1234567890.md"
    );
    assert!(candidates[2].reason.contains("notion_export_filename"));
    assert!(!candidates
        .iter()
        .any(|candidate| candidate.raw_target == "../concept/混合检索.md"));
}

#[test]
fn source_summary_candidates_require_exact_url_or_title_evidence() {
    let sources = vec![
        DbSourceEvidence {
            id: "source-url".into(),
            uri: "file:///notes/source-url.md".into(),
            body: "---\ntitle: URL Source\n---\nBody".into(),
        },
        DbSourceEvidence {
            id: "source-title".into(),
            uri: "file:///notes/source-title.md".into(),
            body: "---\ntitle: Exact Source Title\n---\nBody".into(),
        },
        DbSourceEvidence {
            id: "source-title-fm".into(),
            uri: "file:///notes/source-title-fm.md".into(),
            body: "---\ntitle: Frontmatter Title Evidence\n---\nBody".into(),
        },
    ];
    let summaries = vec![
        DbPageEvidence {
            id: "summary-url".into(),
            title: "Any Summary".into(),
            markdown:
                "---\nentry_type: summary\nsource_url: \"file:///notes/source-url.md\"\n---\nBody"
                    .into(),
            entry_type: Some("summary".into()),
        },
        DbPageEvidence {
            id: "summary-title".into(),
            title: "摘要：Exact Source Title".into(),
            markdown: "---\nentry_type: summary\nsource_title: \"Exact Source Title\"\n---\nBody"
                .into(),
            entry_type: Some("summary".into()),
        },
        DbPageEvidence {
            id: "summary-title-fm".into(),
            title: "Different Summary Title".into(),
            markdown: "---\nentry_type: summary\ntitle: \"Frontmatter Title Evidence\"\n---\nBody"
                .into(),
            entry_type: Some("summary".into()),
        },
    ];

    let candidates = find_source_summary_candidates(&sources, &summaries);

    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[0].source_id, "source-title");
    assert_eq!(candidates[0].summary_page_id, "summary-title");
    assert_eq!(candidates[0].kind, SourceSummaryCandidateKind::ExactTitle);
    assert_eq!(candidates[0].action, "candidate_only");
    assert_eq!(candidates[1].source_id, "source-title-fm");
    assert_eq!(candidates[1].summary_page_id, "summary-title-fm");
    assert_eq!(candidates[1].kind, SourceSummaryCandidateKind::ExactTitle);
    assert_eq!(candidates[2].source_id, "source-url");
    assert_eq!(candidates[2].summary_page_id, "summary-url");
    assert_eq!(candidates[2].kind, SourceSummaryCandidateKind::ExactUrl);
}

#[test]
fn fuzzy_source_summary_matches_are_deferred_for_humans() {
    let sources = vec![DbSourceEvidence {
        id: "source-a".into(),
        uri: "file:///notes/agent-config.md".into(),
        body: "---\ntitle: Agent Config\n---\nBody".into(),
    }];
    let summaries = vec![DbPageEvidence {
        id: "summary-a".into(),
        title: "摘要：Agent Configuration".into(),
        markdown: "---\nentry_type: summary\n---\nBody".into(),
        entry_type: Some("summary".into()),
    }];

    let candidates = find_source_summary_candidates(&sources, &summaries);

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source_id, "source-a");
    assert_eq!(candidates[0].summary_page_id, "summary-a");
    assert_eq!(candidates[0].kind, SourceSummaryCandidateKind::NeedsHuman);
    assert_eq!(candidates[0].action, "deferred");
}

mod cli_coverage {
    use assert_cmd::Command;
    use predicates::str::contains;
    use serde_json::Value;
    use std::path::Path;
    use time::OffsetDateTime;
    use wiki_core::{EntryType, RawArtifact, Scope, WikiPage};
    use wiki_storage::{SqliteRepository, StorageSnapshot, WikiRepository};

    fn wiki_cli() -> Command {
        Command::cargo_bin("wiki-cli").unwrap()
    }

    fn shared_scope() -> Scope {
        Scope::Shared {
            team_id: "wiki".into(),
        }
    }

    fn write_file(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn seed_db(db_path: &Path) -> (String, String) {
        let repo = SqliteRepository::open(db_path).unwrap();
        let source = RawArtifact::new("file:///source-a.md", "Source A body", shared_scope());
        let source_id = source.id.0.to_string();
        let mut page = WikiPage::new("Page A", "Page A body", shared_scope())
            .with_entry_type(EntryType::Summary);
        page.updated_at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let page_id = page.id.0.to_string();
        repo.save_snapshot(&StorageSnapshot {
            sources: vec![source],
            pages: vec![page],
            ..StorageSnapshot::default()
        })
        .unwrap();
        (source_id, page_id)
    }

    #[test]
    fn consistency_audit_writes_timestamped_json_and_chinese_markdown() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("wiki.db");
        let vault = temp.path().join("vault");
        let (_source_id, page_id) = seed_db(&db_path);
        write_file(
            &vault.join("pages/summary/page-a.md"),
            &format!("---\npage_id: \"{page_id}\"\n---\n\nPage A body\n"),
        );
        write_file(&vault.join("sources/raw/source-a.md"), "Source A body\n");

        wiki_cli()
            .arg("--db")
            .arg(&db_path)
            .arg("--wiki-dir")
            .arg(&vault)
            .arg("consistency-audit")
            .assert()
            .success()
            .stdout(contains("consistency_audit db_pages=1 db_sources=1"))
            .stdout(contains("json_report_file="))
            .stdout(contains("markdown_report_file="));

        let reports: Vec<_> = std::fs::read_dir(vault.join("reports"))
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .collect();
        assert!(reports.iter().any(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("consistency-audit-")
                && path.extension().unwrap() == "json"
        }));
        let md_path = reports
            .iter()
            .find(|path| path.extension().unwrap() == "md")
            .unwrap();
        let markdown = std::fs::read_to_string(md_path).unwrap();
        assert!(markdown.contains("# 一致性审计"));
        assert!(markdown.contains("同名 JSON 是机器事实源"));
    }

    #[test]
    fn consistency_audit_scans_only_pages_and_sources_and_reports_zero_byte_unmanaged() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("wiki.db");
        let vault = temp.path().join("vault");
        seed_db(&db_path);
        write_file(&vault.join("reports/empty.md"), "");
        write_file(&vault.join("notes/empty.md"), "");
        write_file(&vault.join("pages/concept/empty.md"), "");
        write_file(&vault.join("sources/raw/source-a.md"), "Source A body\n");

        wiki_cli()
            .arg("--db")
            .arg(&db_path)
            .arg("--wiki-dir")
            .arg(&vault)
            .arg("consistency-audit")
            .assert()
            .success();

        let report_path = std::fs::read_dir(vault.join("reports"))
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().unwrap() == "json")
            .unwrap();
        let json: Value =
            serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
        assert_eq!(
            json["vault"]["empty_unmanaged_files"],
            serde_json::json!(["pages/concept/empty.md"])
        );
        assert!(!json.to_string().contains("reports/empty.md"));
        assert!(!json.to_string().contains("notes/empty.md"));
    }

    #[test]
    fn consistency_audit_reports_palace_missing_page_drawer() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("wiki.db");
        let vault = temp.path().join("vault");
        let palace_path = temp.path().join("palace.db");
        let (_source_id, page_id) = seed_db(&db_path);
        write_file(
            &vault.join("pages/summary/page-a.md"),
            &format!("---\npage_id: \"{page_id}\"\n---\n\nPage A body\n"),
        );
        write_file(&vault.join("sources/raw/source-a.md"), "Source A body\n");
        let conn = rusqlite::Connection::open(&palace_path).unwrap();
        rust_mempalace::db::init_schema(&conn).unwrap();

        wiki_cli()
            .arg("--db")
            .arg(&db_path)
            .arg("--wiki-dir")
            .arg(&vault)
            .arg("--palace")
            .arg(&palace_path)
            .arg("consistency-audit")
            .assert()
            .success()
            .stdout(contains("palace_missing_page_drawers=1"));

        let report_path = std::fs::read_dir(vault.join("reports"))
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().unwrap() == "json")
            .unwrap();
        let json: Value =
            serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
        assert_eq!(
            json["palace"]["missing_page_drawers"],
            serde_json::json!([format!("wiki://page/{page_id}")])
        );
        assert_eq!(
            json["palace"]["source_drawer_policy_note"],
            "source drawers are out of scope; source bodies are not expected in Mempalace"
        );
    }

    #[test]
    fn consistency_audit_does_not_require_ineligible_pages_in_palace() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("wiki.db");
        let vault = temp.path().join("vault");
        let palace_path = temp.path().join("palace.db");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let page = WikiPage::new("Index Page", "Index body", shared_scope())
            .with_entry_type(EntryType::Index);
        let page_id = page.id.0.to_string();
        repo.save_snapshot(&StorageSnapshot {
            pages: vec![page],
            ..StorageSnapshot::default()
        })
        .unwrap();
        write_file(
            &vault.join("pages/index/index-page.md"),
            &format!("---\npage_id: \"{page_id}\"\n---\n\nIndex body\n"),
        );
        let conn = rusqlite::Connection::open(&palace_path).unwrap();
        rust_mempalace::db::init_schema(&conn).unwrap();

        wiki_cli()
            .arg("--db")
            .arg(&db_path)
            .arg("--wiki-dir")
            .arg(&vault)
            .arg("--palace")
            .arg(&palace_path)
            .arg("consistency-audit")
            .assert()
            .success()
            .stdout(contains("palace_missing_page_drawers=0"));
    }

    #[test]
    fn consistency_audit_does_not_mark_db_expected_empty_page_for_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("wiki.db");
        let vault = temp.path().join("vault");
        let repo = SqliteRepository::open(&db_path).unwrap();
        let page =
            WikiPage::new("Empty Expected", "", shared_scope()).with_entry_type(EntryType::Summary);
        repo.save_snapshot(&StorageSnapshot {
            pages: vec![page],
            ..StorageSnapshot::default()
        })
        .unwrap();
        write_file(&vault.join("pages/summary/Empty Expected.md"), "");

        wiki_cli()
            .arg("--db")
            .arg(&db_path)
            .arg("--wiki-dir")
            .arg(&vault)
            .arg("consistency-audit")
            .assert()
            .success()
            .stdout(contains("vault_empty_unmanaged=0"));
    }
}

mod plan_apply_coverage {
    use super::*;
    use consistency::ConsistencyApplyOptions;
    use serde_json::json;
    use std::path::Path;
    use time::OffsetDateTime;
    use wiki_core::{EntryType, RawArtifact, Scope, WikiPage};
    use wiki_kernel::InMemoryStore;
    use wiki_storage::{SqliteRepository, StorageSnapshot, WikiRepository};

    fn shared_scope() -> Scope {
        Scope::Shared {
            team_id: "wiki".into(),
        }
    }

    fn write_file(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn seed_repo(db_path: &Path) -> (SqliteRepository, InMemoryStore, String, String) {
        let repo = SqliteRepository::open(db_path).unwrap();
        let source = RawArtifact::new("file:///notes/source-a.md", "Source A", shared_scope());
        let source_id = source.id.0.to_string();
        let page = WikiPage::new("Summary A", "# Summary A\n", shared_scope())
            .with_entry_type(EntryType::Summary);
        let page_id = page.id.0.to_string();
        let snapshot = StorageSnapshot {
            sources: vec![source],
            pages: vec![page],
            ..StorageSnapshot::default()
        };
        repo.save_snapshot(&snapshot).unwrap();
        let store = InMemoryStore::from_snapshot(snapshot);
        (repo, store, source_id, page_id)
    }

    fn audit_json(vault: &Path, page_id: &str, source_id: &str) -> serde_json::Value {
        json!({
            "version": 1,
            "generated_at": "2026-04-26T00:00:00Z",
            "db_path": "wiki.db",
            "wiki_dir": vault.display().to_string(),
            "palace_path": null,
            "db": {
                "page_count": 1,
                "source_count": 1,
                "page_ids": [page_id],
                "source_ids": [source_id]
            },
            "vault": {
                "scanned_roots": ["pages", "sources"],
                "page_files": 1,
                "source_files": 1,
                "managed_files": [],
                "missing_pages": [],
                "missing_sources": [],
                "extra_pages": [],
                "extra_sources": [],
                "empty_unmanaged_files": ["pages/concept/empty.md"],
                "stale_notion_links": [],
                "unresolved_local_links": [],
                "warnings": []
            },
            "palace": {
                "skipped": true,
                "drawer_count": 0,
                "page_drawer_count": 0,
                "missing_page_drawers": [],
                "stale_page_drawers": [],
                "stale_page_drawer_contents": [],
                "source_drawer_policy_note": "source drawers are out of scope; source bodies are not expected in Mempalace",
                "warnings": []
            },
            "candidates": {
                "source_summary_exact_matches": [{
                    "source_id": source_id,
                    "source_uri": "file:///notes/source-a.md",
                    "source_title": "Source A",
                    "summary_page_id": page_id,
                    "summary_title": "Summary A",
                    "kind": "exact_url",
                    "action": "candidate_only"
                }],
                "source_summary_needs_human": [],
                "safe_cleanup_candidates": ["pages/concept/empty.md"]
            }
        })
    }

    #[test]
    fn consistency_plan_rejects_untimestamped_audit() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let path = vault.join("reports/consistency-audit.json");
        write_file(&path, "{}");

        let err = run_consistency_plan(&path, None, OffsetDateTime::now_utc()).unwrap_err();

        assert!(err.to_string().contains("timestamped consistency-audit"));
    }

    #[test]
    fn consistency_plan_writes_validated_chinese_markdown() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let (_repo, _store, source_id, page_id) = seed_repo(&temp.path().join("wiki.db"));
        let audit_path = vault.join("reports/consistency-audit-20260426T000000Z.json");
        write_file(
            &audit_path,
            &serde_json::to_string_pretty(&audit_json(&vault, &page_id, &source_id)).unwrap(),
        );

        let (plan, files) =
            run_consistency_plan(&audit_path, None, OffsetDateTime::now_utc()).unwrap();

        assert_eq!(plan.actions.len(), 2);
        assert!(plan.actions.iter().any(|action| {
            action.kind == ConsistencyActionKind::VaultCleanup && action.executable
        }));
        assert!(plan
            .actions
            .iter()
            .any(|action| action.kind == ConsistencyActionKind::DbFix && action.executable));
        let markdown = std::fs::read_to_string(files.markdown_path).unwrap();
        assert!(markdown.contains("# 一致性治理计划"));
        assert!(markdown.contains("动作"));
    }

    #[test]
    fn consistency_plan_treats_plain_notion_urls_as_report_only() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let (_repo, _store, source_id, page_id) = seed_repo(&temp.path().join("wiki.db"));
        let mut audit = audit_json(&vault, &page_id, &source_id);
        audit["vault"]["stale_notion_links"] = serde_json::json!([{
            "page_id": page_id,
            "page_title": "Summary A",
            "raw_target": "https://www.notion.so/Example-abcdef1234567890abcdef1234567890?pvs=21",
            "decoded_target": "https://www.notion.so/Example-abcdef1234567890abcdef1234567890?pvs=21",
            "reason": "notion_url",
            "action": "candidate_only"
        }]);
        audit["candidates"]["safe_cleanup_candidates"] = serde_json::json!([]);
        audit["candidates"]["source_summary_exact_matches"] = serde_json::json!([]);
        let audit_path = vault.join("reports/consistency-audit-20260426T000000Z.json");
        write_file(&audit_path, &serde_json::to_string_pretty(&audit).unwrap());

        let (plan, _files) =
            run_consistency_plan(&audit_path, None, OffsetDateTime::now_utc()).unwrap();

        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].kind, ConsistencyActionKind::Deferred);
        assert_eq!(plan.actions[0].operation, "record_legacy_notion_url");
        assert!(!plan.actions[0].executable);
    }

    #[test]
    fn consistency_apply_rejects_plan_path_injection_before_writes() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let (repo, mut store, source_id, page_id) = seed_repo(&temp.path().join("wiki.db"));
        let audit_path = vault.join("reports/consistency-audit-20260426T000000Z.json");
        write_file(
            &audit_path,
            &serde_json::to_string_pretty(&audit_json(&vault, &page_id, &source_id)).unwrap(),
        );
        let plan = ConsistencyPlan {
            version: 1,
            generated_at: "2026-04-26T00:01:00Z".into(),
            audit_report_path: audit_path.display().to_string(),
            wiki_dir: vault.display().to_string(),
            actions: vec![ConsistencyPlanAction {
                kind: ConsistencyActionKind::VaultCleanup,
                path: "pages/not-in-audit.md".into(),
                operation: "delete_unmanaged_empty_file".into(),
                value: None,
                reason: "bad".into(),
                executable: true,
            }],
        };
        let plan_path = vault.join("reports/consistency-plan-20260426T000100Z.json");
        write_file(&plan_path, &serde_json::to_string_pretty(&plan).unwrap());

        let err = run_consistency_apply(
            &repo,
            &mut store,
            &[],
            ConsistencyApplyOptions {
                plan_path: &plan_path,
                wiki_dir: &vault,
                palace_path: None,
                palace_bank_id: "wiki",
                apply: true,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("outside audit evidence"));
    }

    #[test]
    fn consistency_apply_rejects_wiki_dir_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let other_vault = temp.path().join("other-vault");
        std::fs::create_dir_all(&other_vault).unwrap();
        let (repo, mut store, source_id, page_id) = seed_repo(&temp.path().join("wiki.db"));
        let audit_path = vault.join("reports/consistency-audit-20260426T000000Z.json");
        write_file(
            &audit_path,
            &serde_json::to_string_pretty(&audit_json(&vault, &page_id, &source_id)).unwrap(),
        );
        let (_plan, files) =
            run_consistency_plan(&audit_path, None, OffsetDateTime::now_utc()).unwrap();

        let err = run_consistency_apply(
            &repo,
            &mut store,
            &[],
            ConsistencyApplyOptions {
                plan_path: &files.json_path,
                wiki_dir: &other_vault,
                palace_path: None,
                palace_bank_id: "wiki",
                apply: false,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("does not match audit wiki_dir"));
    }

    #[test]
    fn consistency_apply_rejects_untimestamped_audit_in_plan() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let (repo, mut store, source_id, page_id) = seed_repo(&temp.path().join("wiki.db"));
        let audit_path = vault.join("reports/consistency-audit.json");
        write_file(
            &audit_path,
            &serde_json::to_string_pretty(&audit_json(&vault, &page_id, &source_id)).unwrap(),
        );
        let plan = ConsistencyPlan {
            version: 1,
            generated_at: "2026-04-26T00:01:00Z".into(),
            audit_report_path: audit_path.display().to_string(),
            wiki_dir: vault.display().to_string(),
            actions: vec![],
        };
        let plan_path = vault.join("reports/consistency-plan-20260426T000100Z.json");
        write_file(&plan_path, &serde_json::to_string_pretty(&plan).unwrap());

        let err = run_consistency_apply(
            &repo,
            &mut store,
            &[],
            ConsistencyApplyOptions {
                plan_path: &plan_path,
                wiki_dir: &vault,
                palace_path: None,
                palace_bank_id: "wiki",
                apply: false,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("timestamped consistency-audit"));
    }

    #[test]
    fn consistency_apply_dry_run_does_not_write_db_or_vault() {
        let temp = tempfile::tempdir().unwrap();
        let vault = temp.path().join("vault");
        let (repo, mut store, source_id, page_id) = seed_repo(&temp.path().join("wiki.db"));
        let empty = vault.join("pages/concept/empty.md");
        write_file(&empty, "");
        let audit_path = vault.join("reports/consistency-audit-20260426T000000Z.json");
        write_file(
            &audit_path,
            &serde_json::to_string_pretty(&audit_json(&vault, &page_id, &source_id)).unwrap(),
        );
        let (plan, files) =
            run_consistency_plan(&audit_path, None, OffsetDateTime::now_utc()).unwrap();
        let before = store.pages.values().next().unwrap().markdown.clone();

        let report = run_consistency_apply(
            &repo,
            &mut store,
            &[],
            ConsistencyApplyOptions {
                plan_path: &files.json_path,
                wiki_dir: &vault,
                palace_path: None,
                palace_bank_id: "wiki",
                apply: false,
            },
        )
        .unwrap();

        assert_eq!(report.mode, "dry-run");
        assert_eq!(
            plan.actions
                .iter()
                .filter(|action| action.executable)
                .count(),
            2
        );
        assert!(empty.exists());
        assert_eq!(store.pages.values().next().unwrap().markdown, before);
    }
}
