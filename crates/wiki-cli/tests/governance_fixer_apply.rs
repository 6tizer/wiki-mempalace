use assert_cmd::Command;
use predicates::str::contains;
use std::path::Path;
use time::OffsetDateTime;
use wiki_core::{
    DomainSchema, EvidenceFixAction, EvidenceFixActionKind, EvidenceFixActionStatus,
    EvidenceFixPayload, EvidenceFixerApplyReport, EvidenceFixerPlan, EvidenceFixerTombstone,
    PageId, Scope, WikiPage,
};
use wiki_kernel::{LlmWikiEngine, NoopWikiHook};
use wiki_storage::{SqliteRepository, WikiRepository};

fn wiki_cli() -> Command {
    Command::cargo_bin("wiki-cli").unwrap()
}

fn private_scope() -> Scope {
    Scope::Private {
        agent_id: "cli".into(),
    }
}

fn seed_page(db_path: &Path, title: &str, markdown: &str) -> PageId {
    let repo = SqliteRepository::open(db_path).unwrap();
    let mut eng: LlmWikiEngine<NoopWikiHook> =
        LlmWikiEngine::new(DomainSchema::permissive_default());
    let page_id = eng.write_page(WikiPage::new(title, markdown, private_scope()), "test");
    eng.save_to_repo_and_flush_outbox(&repo).unwrap();
    page_id
}

fn retire_plan(page_id: PageId) -> EvidenceFixerPlan {
    let mut plan = EvidenceFixerPlan {
        plan_id: "plan-retire".into(),
        source_scan_report_id: "scan-1".into(),
        generated_at: Some(OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()),
        viewer_scope: Some("private:cli".into()),
        summary: Default::default(),
        actions: vec![EvidenceFixAction {
            action_id: "fix-0001".into(),
            kind: EvidenceFixActionKind::RetirePage,
            status: EvidenceFixActionStatus::Ready,
            subject_type: "page".into(),
            subject_id: Some(page_id.0.to_string()),
            label: Some("Retire".into()),
            evidence: Vec::new(),
            blockers: Vec::new(),
            payload: EvidenceFixPayload::RetirePage {
                tombstone_required: true,
            },
        }],
    };
    plan.refresh_summary();
    plan
}

#[test]
fn fixer_apply_retires_page_and_restore_recovers_it() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("wiki.db");
    let wiki_dir = temp.path().join("vault");
    let report_dir = wiki_dir.join("reports").join("fixer-apply");
    let restore_dir = wiki_dir.join("reports").join("fixer-restore");
    let page_id = seed_page(&db_path, "Retire", "body");
    let plan_path = temp.path().join("plan.json");
    std::fs::write(
        &plan_path,
        serde_json::to_string_pretty(&retire_plan(page_id)).unwrap(),
    )
    .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("--sync-wiki")
        .arg("governance")
        .arg("fixer-apply")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--policy")
        .arg("evidence-auto")
        .arg("--apply")
        .arg("--report-dir")
        .arg("reports/fixer-apply")
        .assert()
        .success()
        .stdout(contains("applied=1"))
        .stdout(contains("tombstone_file="));

    let repo = SqliteRepository::open(&db_path).unwrap();
    let snapshot = repo.load_snapshot().unwrap();
    assert!(!snapshot.pages.iter().any(|page| page.id == page_id));

    let apply_report_path = std::fs::read_dir(&report_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with("evidence-fixer-apply.json")
        })
        .unwrap();
    let apply_report: EvidenceFixerApplyReport =
        serde_json::from_str(&std::fs::read_to_string(apply_report_path).unwrap()).unwrap();
    let tombstone_id = apply_report.tombstones[0].tombstone_id.clone();
    let tombstone_path = report_dir.join(format!("{tombstone_id}.json"));
    let _: EvidenceFixerTombstone =
        serde_json::from_str(&std::fs::read_to_string(&tombstone_path).unwrap()).unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("--wiki-dir")
        .arg(&wiki_dir)
        .arg("--sync-wiki")
        .arg("governance")
        .arg("restore")
        .arg("--tombstone")
        .arg(&tombstone_path)
        .arg("--apply")
        .arg("--report-dir")
        .arg("reports/fixer-restore")
        .assert()
        .success()
        .stdout(contains("status=Applied"));

    let snapshot = repo.load_snapshot().unwrap();
    assert!(snapshot.pages.iter().any(|page| page.id == page_id));
    assert!(restore_dir.exists());
}

#[test]
fn fixer_apply_preflight_does_not_mutate_db() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("wiki.db");
    let page_id = seed_page(&db_path, "Retire", "body");
    let plan_path = temp.path().join("plan.json");
    std::fs::write(
        &plan_path,
        serde_json::to_string_pretty(&retire_plan(page_id)).unwrap(),
    )
    .unwrap();

    wiki_cli()
        .arg("--db")
        .arg(&db_path)
        .arg("governance")
        .arg("fixer-apply")
        .arg("--plan")
        .arg(&plan_path)
        .arg("--policy")
        .arg("evidence-auto")
        .assert()
        .success()
        .stdout(contains("would_apply=1"));

    let repo = SqliteRepository::open(&db_path).unwrap();
    let snapshot = repo.load_snapshot().unwrap();
    assert!(snapshot.pages.iter().any(|page| page.id == page_id));
}
