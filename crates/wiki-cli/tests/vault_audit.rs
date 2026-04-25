#[path = "../src/vault_audit.rs"]
mod vault_audit;

use serde_json::Value;
use std::fs;
use tempfile::tempdir;
use vault_audit::{scan_vault, write_json_and_markdown, write_json_and_markdown_in_vault_reports};

#[test]
fn vault_audit_counts_sources_pages_and_writes_reports() {
    let temp = tempdir().unwrap();
    let vault = temp.path();
    fs::create_dir_all(vault.join("sources/wechat")).unwrap();
    fs::create_dir_all(vault.join("sources/manual")).unwrap();
    fs::create_dir_all(vault.join("pages/summary")).unwrap();
    fs::create_dir_all(vault.join("pages/concept")).unwrap();
    fs::create_dir_all(vault.join("pages/weird")).unwrap();
    fs::create_dir_all(vault.join("reports")).unwrap();
    fs::create_dir_all(vault.join(".wiki")).unwrap();
    fs::create_dir_all(vault.join("notes")).unwrap();

    let source_a = r#"---
title: Source A
kind: source
origin: wechat
compiled_to_wiki: true
created_at: 2026-04-25T00:00:00Z
source_id: source-a
notion_uuid: notion-source-a
tags:
  - alpha
---
Body A
"#;
    let source_b = r#"---
title: Source B
kind: source
origin: manual
compiled_to_wiki: false
created_at: 2026-04-25T00:00:00Z
notion_uuid: notion-source-b
---
Body B
"#;
    let page_summary = r#"---
title: Summary A
entry_type: summary
status: approved
page_id: page-a
notion_uuid: notion-page-a
---
Summary body
"#;
    let page_concept = r#"---
title: Concept A
entry_type: concept
status: draft
notion_uuid: notion-page-b
---
Concept body
"#;
    let page_weird = r#"---
title: Weird A
entry_type: alien
status: draft
---
Weird body
"#;

    fs::write(vault.join("sources/wechat/a.md"), source_a).unwrap();
    fs::write(vault.join("sources/manual/b.md"), source_b).unwrap();
    fs::write(vault.join("sources/root.md"), "no frontmatter\n").unwrap();
    fs::write(vault.join("pages/summary/a.md"), page_summary).unwrap();
    fs::write(vault.join("pages/concept/a.md"), page_concept).unwrap();
    fs::write(vault.join("pages/weird/a.md"), page_weird).unwrap();
    fs::write(vault.join("reports/old.md"), "# old report\n").unwrap();
    fs::write(vault.join("notes/free.md"), "# free note\n").unwrap();
    fs::write(vault.join(".wiki/orphan-audit.json"), "{}\n").unwrap();
    fs::write(vault.join("index.md"), "# Index\n").unwrap();

    let before = fs::read_to_string(vault.join("sources/wechat/a.md")).unwrap();
    let report = scan_vault(vault).unwrap();

    assert_eq!(report.totals.total_files, 10);
    assert_eq!(report.totals.markdown_files, 9);
    assert_eq!(report.totals.source_files, 3);
    assert_eq!(report.totals.page_files, 3);
    assert_eq!(report.totals.report_files, 1);
    assert_eq!(report.totals.wiki_artifact_files, 1);
    assert_eq!(report.totals.old_orphan_audit_files, 1);

    assert_eq!(report.sources.by_origin["wechat"], 1);
    assert_eq!(report.sources.by_origin["manual"], 1);
    assert_eq!(report.sources.by_origin["missing"], 1);
    assert_eq!(report.sources.compiled_to_wiki.true_count, 1);
    assert_eq!(report.sources.compiled_to_wiki.false_count, 1);
    assert_eq!(report.sources.compiled_to_wiki.missing, 1);
    assert_eq!(report.sources.with_source_id, 1);
    assert_eq!(report.sources.with_notion_uuid, 2);

    assert_eq!(report.pages.by_entry_type["summary"], 1);
    assert_eq!(report.pages.by_entry_type["concept"], 1);
    assert_eq!(report.pages.by_entry_type["alien"], 1);
    assert_eq!(report.pages.unsupported_entry_type, 1);
    assert_eq!(report.pages.unsupported_directory, 1);
    assert_eq!(report.pages.with_page_id, 1);
    assert_eq!(report.pages.with_notion_uuid, 2);

    assert_eq!(report.readiness.ready_sources, 2);
    assert_eq!(report.readiness.ready_pages, 2);
    assert_eq!(report.readiness.missing_stable_id, 2);
    assert!(report
        .orphan_candidates
        .by_category
        .contains_key("unsupported_page_directory"));
    assert!(report
        .orphan_candidates
        .by_category
        .contains_key("unclassified_markdown"));
    assert_eq!(report.orphan_candidates.total_files, 3);

    let files = write_json_and_markdown(&report, vault.join("reports/audit")).unwrap();
    assert!(files.json_path.exists());
    assert!(files.markdown_path.exists());
    let json: Value = serde_json::from_str(&fs::read_to_string(files.json_path).unwrap()).unwrap();
    assert_eq!(json["totals"]["source_files"], 3);
    assert_eq!(json["sources"]["by_origin"]["wechat"], 1);
    let markdown = fs::read_to_string(files.markdown_path).unwrap();
    assert!(markdown.contains("# Vault Audit"));
    assert!(markdown.contains("source_of_truth: `vault-audit.json`"));

    let default_files = write_json_and_markdown_in_vault_reports(&report).unwrap();
    assert_eq!(
        default_files.json_path,
        vault.join("reports/vault-audit.json")
    );
    let outside = write_json_and_markdown(&report, temp.path().join("outside"));
    assert!(outside.is_err());

    let after = fs::read_to_string(vault.join("sources/wechat/a.md")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn vault_audit_reports_duplicate_identity_candidates() {
    let temp = tempdir().unwrap();
    let vault = temp.path();
    fs::create_dir_all(vault.join("sources/wechat")).unwrap();

    let source = |title: &str| {
        format!(
            r#"---
title: {title}
kind: source
origin: wechat
compiled_to_wiki: true
created_at: 2026-04-25T00:00:00Z
source_id: duplicate-source
notion_uuid: duplicate-notion
---
Body
"#
        )
    };
    fs::write(vault.join("sources/wechat/a.md"), source("A")).unwrap();
    fs::write(vault.join("sources/wechat/b.md"), source("B")).unwrap();

    let report = scan_vault(vault).unwrap();

    assert_eq!(report.identities.duplicate_source_ids.len(), 1);
    assert_eq!(report.identities.duplicate_notion_uuids.len(), 1);
    assert_eq!(report.readiness.duplicate_identity_candidate_files, 2);
    assert_eq!(report.readiness.ready_sources, 0);
}

#[test]
fn duplicate_readiness_uses_full_path_set_not_report_samples() {
    let temp = tempdir().unwrap();
    let vault = temp.path();
    fs::create_dir_all(vault.join("sources/wechat")).unwrap();

    for index in 0..12 {
        fs::write(
            vault.join(format!("sources/wechat/{index}.md")),
            format!(
                r#"---
title: Source {index}
kind: source
origin: wechat
compiled_to_wiki: true
created_at: 2026-04-25T00:00:00Z
source_id: duplicate-source
---
Body
"#
            ),
        )
        .unwrap();
    }

    let report = scan_vault(vault).unwrap();

    assert_eq!(report.identities.duplicate_source_ids.len(), 1);
    assert_eq!(report.identities.duplicate_source_ids[0].count, 12);
    assert_eq!(report.identities.duplicate_source_ids[0].paths.len(), 10);
    assert_eq!(report.readiness.duplicate_identity_candidate_files, 12);
    assert_eq!(report.readiness.ready_sources, 0);
}

#[test]
fn invalid_utf8_markdown_is_counted_in_source_and_page_stats() {
    let temp = tempdir().unwrap();
    let vault = temp.path();
    fs::create_dir_all(vault.join("sources/wechat")).unwrap();
    fs::create_dir_all(vault.join("pages/summary")).unwrap();

    fs::write(vault.join("sources/wechat/bad.md"), [0xff, 0xfe]).unwrap();
    fs::write(vault.join("pages/summary/bad.md"), [0xff, 0xfe]).unwrap();

    let report = scan_vault(vault).unwrap();

    assert_eq!(report.totals.markdown_files, 2);
    assert_eq!(report.totals.source_files, 1);
    assert_eq!(report.totals.page_files, 1);
    assert_eq!(report.frontmatter.invalid_utf8, 2);
    assert_eq!(report.sources.total, 1);
    assert_eq!(report.sources.invalid_utf8, 1);
    assert_eq!(report.sources.compiled_to_wiki.missing, 1);
    assert_eq!(report.pages.total, 1);
    assert_eq!(report.pages.invalid_utf8, 1);
    assert_eq!(report.pages.missing_entry_type, 1);
    assert_eq!(report.orphan_candidates.by_category["invalid_utf8"], 2);
}
