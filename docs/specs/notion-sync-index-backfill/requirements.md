# Requirements: Notion Sync Index Backfill

## Functional Requirements

- `notion_page_exists` and index insertion must canonicalize page IDs by
  removing hyphens and lowercasing.
- Add `wiki-cli notion-sync-index-backfill`.
- The command scans `<vault>/sources/**/*.md`.
- It reads `source_id`, `notion_uuid`, and origin from frontmatter or source
  directory.
- It maps `origin: x` to `x_bookmark` and `origin: wechat` to `wechat`.
- Default mode is dry-run; `--apply` writes to `notion_page_index`.
- Missing/invalid `source_id`, missing `notion_uuid`, and unknown origin are
  skipped and counted.

## Non-Goals

- No Notion API calls.
- No cursor update.
- No vault file mutation.
- No source content update semantics.
