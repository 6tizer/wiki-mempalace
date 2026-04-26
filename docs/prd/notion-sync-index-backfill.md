# PRD: Notion Sync Index Backfill

**Status**: active  
**Related**: `Notion Incremental Sync`

## Goal

Prevent the first production `notion-sync` run from re-ingesting historical
Notion pages already imported by the offline migration.

## Problem

Historical vault sources already have `notion_uuid` and `source_id`, but the new
`notion_page_index` table is empty. Notion API returns page IDs with hyphens,
while historical frontmatter stores `notion_uuid` without hyphens. Current exact
matching treats existing pages as new.

## Scope

- Normalize Notion page IDs before checking/inserting `notion_page_index`.
- Add a dry-run-first CLI helper to backfill `notion_page_index` from existing
  vault source frontmatter.
- Do not run production `notion-sync --db-id all` in this PR.

## Success Criteria

- Hyphenated and non-hyphenated Notion page IDs match the same index row.
- Historical source frontmatter can populate `notion_page_index` without
  mutating vault files.
- Default mode is dry-run; `--apply` is required to write index rows.
