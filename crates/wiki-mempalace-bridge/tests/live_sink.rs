#![cfg(feature = "live")]

use std::path::PathBuf;
use wiki_core::{EntryType, Scope, SourceId, WikiPage};
use wiki_mempalace_bridge::{LiveMempalaceSink, MempalaceWikiSink};

fn temp_db_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "wiki-mempalace-bridge-{name}-{}.db",
        uuid::Uuid::new_v4()
    ));
    path
}

fn count_drawers(path: &PathBuf) -> i64 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn page_written_creates_drawer_and_rerun_does_not_duplicate() {
    let path = temp_db_path("page-written");
    let sink = LiveMempalaceSink::open(&path, "wiki").unwrap();
    let mut page = WikiPage::new(
        "Palace Page",
        "# Palace Page\n\neligible page body",
        Scope::Shared {
            team_id: "wiki".into(),
        },
    );
    page.entry_type = Some(EntryType::Concept);

    sink.on_page_written(&page).unwrap();
    sink.on_page_written(&page).unwrap();

    let conn = rusqlite::Connection::open(&path).unwrap();
    let row: (i64, String, String, String) = conn
        .query_row(
            "SELECT COUNT(*), source_path, content, bank_id FROM drawers",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(row.0, 1);
    assert_eq!(row.1, format!("wiki://page/{}", page.id.0));
    assert_eq!(row.2, page.markdown);
    assert_eq!(row.3, "wiki");

    let _ = std::fs::remove_file(path);
}

#[test]
fn skill_page_written_creates_drawer() {
    let path = temp_db_path("skill-page-written");
    let sink = LiveMempalaceSink::open(&path, "wiki").unwrap();
    let mut page = WikiPage::new(
        "Skill Page",
        "## 触发条件\n需要复用操作技能",
        Scope::Shared {
            team_id: "wiki".into(),
        },
    );
    page.entry_type = Some(EntryType::Skill);

    sink.on_page_written(&page).unwrap();

    assert_eq!(count_drawers(&path), 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn same_page_content_can_exist_in_different_banks() {
    let path = temp_db_path("cross-bank-page-written");
    let sink_a = LiveMempalaceSink::open(&path, "bank_a").unwrap();
    let sink_b = LiveMempalaceSink::open(&path, "bank_b").unwrap();
    let mut page = WikiPage::new(
        "Shared Coordinates",
        "# Shared Coordinates\n\nsame body",
        Scope::Shared {
            team_id: "bank_a".into(),
        },
    );
    page.entry_type = Some(EntryType::Concept);

    sink_a.on_page_written(&page).unwrap();
    sink_b.on_page_written(&page).unwrap();
    sink_a.on_page_written(&page).unwrap();

    let conn = rusqlite::Connection::open(&path).unwrap();
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))
        .unwrap();
    let bank_a: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM drawers WHERE bank_id = 'bank_a'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let bank_b: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM drawers WHERE bank_id = 'bank_b'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(bank_a, 1);
    assert_eq!(bank_b, 1);

    let _ = std::fs::remove_file(path);
}

#[test]
fn source_ingested_does_not_create_source_drawer() {
    let path = temp_db_path("source-ingested");
    let sink = LiveMempalaceSink::open(&path, "wiki").unwrap();

    sink.on_source_ingested(SourceId(uuid::Uuid::new_v4()))
        .unwrap();

    assert_eq!(count_drawers(&path), 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn ineligible_pages_do_not_create_drawers() {
    let path = temp_db_path("ineligible-page");
    let sink = LiveMempalaceSink::open(&path, "wiki").unwrap();
    let mut page = WikiPage::new(
        "Lint Report",
        "# Lint Report\n\nnoise",
        Scope::Shared {
            team_id: "wiki".into(),
        },
    );
    page.entry_type = Some(EntryType::LintReport);

    sink.on_page_written(&page).unwrap();

    assert_eq!(count_drawers(&path), 0);
    let _ = std::fs::remove_file(path);
}
