# Tasks: Notion Sync Index Backfill

- [x] Add storage Notion page ID canonicalization and tests.
- [x] Add CLI planner/apply helper and CLI tests.
- [x] Add `notion-sync-index-backfill` command.
- [x] Verify dry-run does not insert rows.
- [x] Verify apply inserts canonical rows and rerun is idempotent.
- [x] Run focused tests.
- [x] Run staging close-loop on copied production DB: backfill applied 1099
      index rows to the copy, then `notion-sync --db-id all --dry-run`
      reported `skipped=1095`, `new=171`, `errors=0`; production DB remained
      unchanged.
- [x] Run production index-only backfill after DB backup: backup stored at
      `/Users/mac-mini/Documents/wiki-backups/notion-index-backfill-20260426-214454/wiki.db`;
      production `notion_page_index` now has 1099 rows, `notion_sync_cursors`
      remains 0, and production `notion-sync --db-id all --dry-run` reports
      `skipped=1095`, `new=171`, `errors=0`.
