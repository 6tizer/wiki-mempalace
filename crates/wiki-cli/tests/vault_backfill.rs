#[path = "../src/vault_backfill.rs"]
mod vault_backfill;

use std::path::Path;

use vault_backfill::{
    backfill_vault, backfill_vault_with_repo, backfill_vault_with_scope_str, parse_scope,
    BackfillMode, VaultBackfillOptions,
};
use wiki_core::WikiEvent;
use wiki_storage::{SqliteRepository, StorageSnapshot, WikiRepository};

fn write_file(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

fn source_body() -> &'static str {
    r#"---
title: "Source A"
notion_uuid: "11111111-1111-1111-1111-111111111111"
url: "https://example.com/a"
tags: "alpha, beta"
compiled_to_wiki: false
---

# Source A

Historical source body.
"#
}

fn page_body() -> &'static str {
    r#"---
title: "Concept A"
notion_uuid: "22222222-2222-2222-2222-222222222222"
entry_type: concept
status: approved
tags: "alpha"
---

# Concept A

[[Related]]
"#
}

fn page_body_with_id(page_id: &str) -> String {
    page_body().replacen("---\n", &format!("---\npage_id: \"{page_id}\"\n"), 1)
}

fn run_backfill(
    vault: &Path,
    db_path: &Path,
    report_dir: &Path,
    mode: BackfillMode,
) -> vault_backfill::Result<vault_backfill::BackfillReport> {
    backfill_vault(VaultBackfillOptions {
        vault_path: vault.to_path_buf(),
        db_path: db_path.to_path_buf(),
        scope: wiki_core::Scope::Shared {
            team_id: "wiki".to_string(),
        },
        mode,
        limit: None,
        report_dir: report_dir.to_path_buf(),
    })
}

#[test]
fn parse_scope_accepts_private_and_shared_forms() {
    assert_eq!(
        parse_scope("shared:wiki").unwrap(),
        wiki_core::Scope::Shared {
            team_id: "wiki".to_string()
        }
    );
    assert_eq!(
        parse_scope("private:agent1").unwrap(),
        wiki_core::Scope::Private {
            agent_id: "agent1".to_string()
        }
    );
    assert!(parse_scope("wiki").is_err());
}

#[test]
fn dry_run_reports_without_mutating_vault_or_db() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    let source = vault.join("sources/source-a.md");
    let page = vault.join("pages/concept/concept-a.md");
    write_file(&source, source_body());
    write_file(&page, page_body());

    let before_source = std::fs::read_to_string(&source).unwrap();
    let before_page = std::fs::read_to_string(&page).unwrap();

    let report = run_backfill(&vault, &db_path, &report_dir, BackfillMode::DryRun).unwrap();

    assert_eq!(report.sources_seen, 1);
    assert_eq!(report.pages_seen, 1);
    assert_eq!(report.source_id_writes_planned, 1);
    assert_eq!(report.page_id_writes_planned, 1);
    assert_eq!(report.records.len(), 2);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), before_source);
    assert_eq!(std::fs::read_to_string(&page).unwrap(), before_page);
    assert!(!db_path.exists(), "dry-run must not create db");
    assert!(report_dir.join("vault-backfill-report.json").exists());
    assert!(report_dir.join("vault-backfill-report.md").exists());
}

#[test]
fn rerun_repairs_missing_page_written_after_interrupted_apply() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    let page_id = uuid::Uuid::parse_str("33333333-3333-5333-8333-333333333333").unwrap();
    write_file(
        &vault.join("pages/concept/concept-a.md"),
        &page_body_with_id(&page_id.to_string()),
    );

    let repo = SqliteRepository::open(&db_path).unwrap();
    repo.save_snapshot(&StorageSnapshot {
        pages: vec![wiki_core::WikiPage {
            id: wiki_core::PageId(page_id),
            title: "Concept A".to_string(),
            markdown: "# Concept A".to_string(),
            scope: wiki_core::Scope::Shared {
                team_id: "wiki".to_string(),
            },
            updated_at: time::OffsetDateTime::now_utc(),
            outbound_page_titles: Vec::new(),
            entry_type: Some(wiki_core::EntryType::Concept),
            status: wiki_core::EntryStatus::Approved,
            created_at: Some(time::OffsetDateTime::now_utc()),
            status_entered_at: Some(time::OffsetDateTime::now_utc()),
        }],
        ..StorageSnapshot::default()
    })
    .unwrap();

    let report = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();

    assert_eq!(report.pages_imported, 0);
    assert_eq!(report.page_written_events, 1);
    assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 1);

    let rerun = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();
    assert_eq!(rerun.page_written_events, 0);
    assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 1);
}

#[test]
fn duplicate_page_id_is_skipped_without_collapsing_pages() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    let page_id = uuid::Uuid::parse_str("44444444-4444-5444-8444-444444444444").unwrap();
    write_file(
        &vault.join("pages/concept/concept-a.md"),
        &page_body_with_id(&page_id.to_string()),
    );
    write_file(
        &vault.join("pages/concept/concept-b.md"),
        &page_body_with_id(&page_id.to_string()),
    );

    let report = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();

    assert_eq!(report.pages_seen, 0);
    assert_eq!(report.skipped.len(), 2);
    assert!(report
        .skipped
        .iter()
        .all(|skip| skip.reason.contains("duplicate page_id")));
    assert_eq!(report.page_written_events, 0);
    let repo = SqliteRepository::open(&db_path).unwrap();
    assert_eq!(repo.load_snapshot().unwrap().pages.len(), 0);
    assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 0);
}

#[test]
fn duplicate_source_id_is_skipped_without_collapsing_sources() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    let source_id = "66666666-6666-5666-8666-666666666666";
    let source_with_id =
        source_body().replacen("---\n", &format!("---\nsource_id: \"{source_id}\"\n"), 1);
    write_file(&vault.join("sources/source-a.md"), &source_with_id);
    write_file(&vault.join("sources/source-b.md"), &source_with_id);

    let report = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();

    assert_eq!(report.sources_seen, 0);
    assert_eq!(report.skipped.len(), 2);
    assert!(report
        .skipped
        .iter()
        .all(|skip| skip.reason.contains("duplicate source_id")));
    let repo = SqliteRepository::open(&db_path).unwrap();
    assert_eq!(repo.load_snapshot().unwrap().sources.len(), 0);
    assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 0);
}

#[test]
fn rerun_repairs_existing_records_with_same_id() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    let page_id = uuid::Uuid::parse_str("55555555-5555-5555-8555-555555555555").unwrap();
    write_file(
        &vault.join("pages/concept/concept-a.md"),
        &page_body_with_id(&page_id.to_string()),
    );

    let repo = SqliteRepository::open(&db_path).unwrap();
    repo.save_snapshot(&StorageSnapshot {
        pages: vec![wiki_core::WikiPage {
            id: wiki_core::PageId(page_id),
            title: "stale title".to_string(),
            markdown: "stale body".to_string(),
            scope: wiki_core::Scope::Private {
                agent_id: "mcp".to_string(),
            },
            updated_at: time::OffsetDateTime::now_utc(),
            outbound_page_titles: Vec::new(),
            entry_type: Some(wiki_core::EntryType::Qa),
            status: wiki_core::EntryStatus::Draft,
            created_at: Some(time::OffsetDateTime::now_utc()),
            status_entered_at: Some(time::OffsetDateTime::now_utc()),
        }],
        ..StorageSnapshot::default()
    })
    .unwrap();

    let report = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();

    assert_eq!(report.pages_imported, 0);
    assert_eq!(report.pages_updated, 1);
    let snapshot = repo.load_snapshot().unwrap();
    let page = &snapshot.pages[0];
    assert_eq!(page.title, "Concept A");
    assert_eq!(
        page.scope,
        wiki_core::Scope::Shared {
            team_id: "wiki".into()
        }
    );
    assert_eq!(page.entry_type, Some(wiki_core::EntryType::Concept));
    assert_eq!(page.status, wiki_core::EntryStatus::Approved);
    assert!(page.markdown.contains("[[Related]]"));
}

#[test]
fn scope_string_entrypoint_dry_run_does_not_create_db() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    write_file(&vault.join("sources/source-a.md"), source_body());

    let report = backfill_vault_with_scope_str(
        &vault,
        &db_path,
        "shared:wiki",
        BackfillMode::DryRun,
        None,
        &report_dir,
    )
    .unwrap();

    assert_eq!(report.sources_seen, 1);
    assert!(!db_path.exists());
}

#[test]
fn repo_entrypoint_apply_imports_into_existing_repo() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    write_file(&vault.join("pages/concept/concept-a.md"), page_body());

    let repo = SqliteRepository::open(&db_path).unwrap();
    let report = backfill_vault_with_repo(
        &vault,
        &repo,
        wiki_core::Scope::Shared {
            team_id: "wiki".to_string(),
        },
        BackfillMode::Apply,
        None,
        &report_dir,
    )
    .unwrap();

    assert_eq!(report.pages_imported, 1);
    assert_eq!(repo.load_snapshot().unwrap().pages.len(), 1);
    assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 1);
}

#[test]
fn apply_writes_missing_ids_imports_db_and_emits_page_written() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    let source = vault.join("sources/source-a.md");
    let page = vault.join("pages/concept/concept-a.md");
    write_file(&source, source_body());
    write_file(&page, page_body());

    let report = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();

    assert_eq!(report.source_id_writes_applied, 1);
    assert_eq!(report.page_id_writes_applied, 1);
    let source_after = std::fs::read_to_string(&source).unwrap();
    let page_after = std::fs::read_to_string(&page).unwrap();
    assert!(source_after.contains("source_id: \""));
    assert!(page_after.contains("page_id: \""));
    assert!(source_after.contains("notion_uuid: \"11111111-1111-1111-1111-111111111111\""));
    assert!(page_after.contains("notion_uuid: \"22222222-2222-2222-2222-222222222222\""));

    let repo = SqliteRepository::open(&db_path).unwrap();
    let snapshot = repo.load_snapshot().unwrap();
    assert_eq!(snapshot.sources.len(), 1);
    assert_eq!(snapshot.pages.len(), 1);
    assert_eq!(
        snapshot.sources[0].scope,
        wiki_core::Scope::Shared {
            team_id: "wiki".to_string()
        }
    );
    assert_eq!(
        snapshot.pages[0].entry_type,
        Some(wiki_core::EntryType::Concept)
    );
    assert_eq!(snapshot.pages[0].status, wiki_core::EntryStatus::Approved);

    let outbox = repo.export_outbox_ndjson().unwrap();
    let events: Vec<WikiEvent> = outbox
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], WikiEvent::PageWritten { .. }));
}

#[test]
fn rerun_apply_is_idempotent_for_records_frontmatter_and_outbox() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("vault");
    let db_path = temp.path().join("wiki.db");
    let report_dir = temp.path().join("reports");
    let source = vault.join("sources/source-a.md");
    let page = vault.join("pages/concept/concept-a.md");
    write_file(&source, source_body());
    write_file(&page, page_body());

    let first = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();
    let source_after_first = std::fs::read_to_string(&source).unwrap();
    let page_after_first = std::fs::read_to_string(&page).unwrap();
    let second = run_backfill(&vault, &db_path, &report_dir, BackfillMode::Apply).unwrap();

    assert_eq!(first.sources_imported, 1);
    assert_eq!(first.pages_imported, 1);
    assert_eq!(second.source_id_writes_applied, 0);
    assert_eq!(second.page_id_writes_applied, 0);
    assert_eq!(second.sources_imported, 0);
    assert_eq!(second.pages_imported, 0);
    assert_eq!(
        std::fs::read_to_string(&source).unwrap(),
        source_after_first
    );
    assert_eq!(std::fs::read_to_string(&page).unwrap(), page_after_first);

    let repo = SqliteRepository::open(&db_path).unwrap();
    let snapshot = repo.load_snapshot().unwrap();
    assert_eq!(snapshot.sources.len(), 1);
    assert_eq!(snapshot.pages.len(), 1);
    assert_eq!(repo.export_outbox_ndjson().unwrap().lines().count(), 1);
}
