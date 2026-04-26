# Notion Source Vault Projection Handoff

## Status

PR #42 merged. Production apply completed for `/Users/mac-mini/Documents/wiki`.

## What changed

- `notion-sync` writes DB-backed `notion://` raw sources and can refresh existing records with Notion page block body text.
- `notion-source-vault-sync` projects those DB sources to Obsidian-visible Markdown under:
  - `sources/x/`
  - `sources/wechat/`
- Source tags are projected in an Obsidian-safe form.
- automation `notion-sync` refreshes existing source bodies/tags inside the cursor window.

## Production evidence

- Backup before sample refresh:
  `/Users/mac-mini/Documents/wiki-backups/wiki-before-notion-refresh-sample-20260427-014921`
- 30-day refresh command used `--since 2026-03-25T00:00:00Z --refresh-existing`.
- X result: fetched 782, new 0, refreshed 108, skipped 674, errors 0.
- WeChat result: fetched 485, new 0, refreshed 53, skipped 432, errors 0.
- Projection apply: notion_sources_seen 176, planned 158, applied 158, existing 176.
- Final dry-run with `--refresh-existing --repair-tags`: planned 0, applied 0, tags_rewritten 0.
- DB-backed Notion source body health: 176 total, 171 with body length >= 1000 chars, invalid Obsidian tags 0.

## Boundaries

- Raw sources are now visible in Obsidian, but they are not compiled wiki pages.
- Mempalace is not expected to contain raw Notion source drawers from this work.
- Archived Notion source retirement remains separate.

## Next workflow

Start a new PRD/spec flow for Notion Source Compilation:

1. Select a small sample of `compiled_to_wiki: false` Notion sources.
2. Run compilation into wiki pages through the normal engine path.
3. Verify source/page links, query results, outbox, and mempalace consumption.
4. Scale to a larger batch only after the sample is clean.
