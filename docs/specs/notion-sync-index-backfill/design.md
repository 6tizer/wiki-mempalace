# Design: Notion Sync Index Backfill

## Flow

`wiki-cli notion-sync-index-backfill --vault <PATH> [--apply]`

1. Walk `<vault>/sources/**/*.md`.
2. Parse simple YAML frontmatter scalar values.
3. Build candidate `(notion_uuid, db_id, source_id)`.
4. Check `notion_page_index` using canonical ID.
5. In dry-run, report planned rows only.
6. In apply, insert missing rows using storage batch insert.

## ID Canonicalization

Storage owns canonicalization so every caller is protected:

- input: `1a970107-4b68-8103-b989-fbd0cfb8343a`
- stored/matched: `1a9701074b688103b989fbd0cfb8343a`

## Output

stdout prints one summary line:

```text
notion_sync_index_backfill mode=dry_run sources_seen=N planned=N applied=N existing=N skipped_missing_notion_uuid=N skipped_missing_source_id=N skipped_unknown_origin=N skipped_invalid_source_id=N
```
