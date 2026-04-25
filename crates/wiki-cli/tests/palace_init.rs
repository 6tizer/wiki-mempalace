#![allow(dead_code)]

#[path = "../src/palace_init.rs"]
mod palace_init;

use std::collections::HashMap;
use std::path::PathBuf;
use wiki_core::{EntryType, PageId, Scope, SourceId, WikiEvent, WikiPage};
use wiki_mempalace_bridge::OutboxResolver;

fn temp_db_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "wiki-cli-palace-init-{name}-{}.db",
        uuid::Uuid::new_v4()
    ));
    path
}

#[derive(Default)]
struct TestResolver {
    pages: HashMap<PageId, WikiPage>,
}

impl OutboxResolver for TestResolver {
    fn claim(&self, _id: wiki_core::ClaimId) -> Option<wiki_core::Claim> {
        None
    }

    fn source_scope(&self, _id: SourceId) -> Option<Scope> {
        None
    }

    fn page(&self, id: PageId) -> Option<WikiPage> {
        self.pages.get(&id).cloned()
    }
}

#[test]
fn shared_wiki_viewer_scope_maps_to_wiki_bank() {
    assert_eq!(
        palace_init::mempalace_bank_from_viewer_scope("shared:wiki"),
        "wiki"
    );
}

#[test]
fn init_core_skips_when_progress_is_at_head() {
    let repo = palace_init::FakePalaceInitRepository::new("", 7, Some(7));
    let sink = palace_init::NoopPalaceInitSink;
    let resolver = palace_init::NoopPalaceInitResolver;
    let report =
        palace_init::run_palace_init_core(&repo, &sink, &resolver, "mempalace", 0).unwrap();

    assert_eq!(report.start_id, 7);
    assert_eq!(report.acked, 0);
    assert_eq!(report.dispatch.lines_seen, 0);
    assert_eq!(repo.acked_up_to_id(), None);
}

#[test]
fn live_init_consumes_page_written_acks_and_uses_shared_wiki_bank() {
    let mut page = WikiPage::new(
        "Init Page",
        "# Init Page\n\npalace init body",
        Scope::Shared {
            team_id: "wiki".into(),
        },
    );
    page.entry_type = Some(EntryType::Summary);
    let line = serde_json::to_string(&WikiEvent::PageWritten {
        page_id: page.id,
        at: time::OffsetDateTime::now_utc(),
    })
    .unwrap();
    let repo = palace_init::FakePalaceInitRepository::new(&line, 1, None);
    let mut resolver = TestResolver::default();
    resolver.pages.insert(page.id, page.clone());
    let palace_path = temp_db_path("live");

    let report = palace_init::run_live_palace_init(
        &repo,
        &resolver,
        &palace_path,
        "shared:wiki",
        "mempalace",
        0,
    )
    .unwrap();

    assert_eq!(report.start_id, 0);
    assert_eq!(report.head_id, 1);
    assert_eq!(report.acked, 1);
    assert_eq!(report.bank_id.as_deref(), Some("wiki"));
    assert_eq!(report.dispatch.by_event["PageWritten"].dispatched, 1);
    assert_eq!(report.drawer_count, Some(1));
    assert!(report.validation.as_ref().unwrap().query_ok);
    assert!(report.validation.as_ref().unwrap().explain_ok);
    assert!(report.validation.as_ref().unwrap().fusion_ok);
    assert_eq!(repo.acked_up_to_id(), Some(1));

    let conn = rusqlite::Connection::open(&palace_path).unwrap();
    let bank: String = conn
        .query_row("SELECT bank_id FROM drawers", [], |row| row.get(0))
        .unwrap();
    assert_eq!(bank, "wiki");

    let _ = std::fs::remove_file(palace_path);
}

#[test]
fn live_init_does_not_ack_when_validation_fails() {
    let mut page = WikiPage::new(
        "Bad Page",
        "x",
        Scope::Shared {
            team_id: "wiki".into(),
        },
    );
    page.entry_type = Some(EntryType::Summary);
    let line = serde_json::to_string(&WikiEvent::PageWritten {
        page_id: page.id,
        at: time::OffsetDateTime::now_utc(),
    })
    .unwrap();
    let repo = palace_init::FakePalaceInitRepository::new(&line, 1, None);
    let mut resolver = TestResolver::default();
    resolver.pages.insert(page.id, page);
    let palace_path = temp_db_path("validation-fail");

    let err = palace_init::run_live_palace_init(
        &repo,
        &resolver,
        &palace_path,
        "shared:wiki",
        "mempalace",
        0,
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("validation failed"));
    assert_eq!(repo.acked_up_to_id(), None);
    let _ = std::fs::remove_file(palace_path);
}

#[test]
fn init_core_does_not_ack_unresolved_required_events() {
    let page_id = PageId(uuid::Uuid::new_v4());
    let line = serde_json::to_string(&WikiEvent::PageWritten {
        page_id,
        at: time::OffsetDateTime::now_utc(),
    })
    .unwrap();
    let repo = palace_init::FakePalaceInitRepository::new(&line, 1, None);
    let sink = palace_init::NoopPalaceInitSink;
    let resolver = palace_init::NoopPalaceInitResolver;

    let err = palace_init::run_palace_init_core(&repo, &sink, &resolver, "mempalace", 0)
        .unwrap_err()
        .to_string();

    assert!(err.contains("unresolved"));
    assert_eq!(repo.acked_up_to_id(), None);
}

#[test]
fn report_writer_outputs_json_and_markdown() {
    let temp = tempfile::tempdir().unwrap();
    let report = palace_init::PalaceInitReport {
        consumer_tag: "mempalace".into(),
        start_id: 0,
        head_id: 1,
        acked: 1,
        bank_id: Some("wiki".into()),
        dispatch: wiki_mempalace_bridge::OutboxDispatchStats::default(),
        drawer_count: Some(0),
        kg_fact_count: Some(0),
        validation: Some(palace_init::PalaceInitValidation {
            sample_query: "wiki".into(),
            query_ok: true,
            explain_ok: true,
            fusion_ok: true,
            bm25_count: 0,
            vector_count: 0,
            graph_count: 0,
        }),
    };

    let files = palace_init::write_report_files(temp.path(), &report).unwrap();

    assert!(files.json_path.exists());
    assert!(files.markdown_path.exists());
    assert!(std::fs::read_to_string(files.markdown_path)
        .unwrap()
        .contains("fusion_ok: true"));
}
