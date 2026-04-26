# PRD: Notion Source Vault Projection

**Status**: active
**Related**: `Notion Incremental Sync`, `Vault Backfill + Palace Init`

## Goal

Make Notion API synced sources visible in the Obsidian vault under
`sources/{origin}/`, matching the existing vault standards.

## Problem

`notion-sync` correctly writes new `notion://` records into `wiki.db` and emits
outbox events, but it does not create source Markdown files. The user primarily
inspects `/Users/mac-mini/Documents/wiki` through Obsidian, so these records are
effectively invisible.

Production evidence on 2026-04-26:

- `wiki.db` contains 176 `notion://` sources.
- `x_bookmark`: 113 sources.
- `wechat`: 63 sources.
- Vault source files matching those `source_id` / `notion_uuid`: 0.

## Scope

- Add a vault projection helper for DB-backed `notion://` sources.
- Add `wiki-cli notion-source-vault-sync` with dry-run default and explicit
  `--apply`.
- Wire `notion-sync --sync-wiki` to project missing source Markdown after a
  successful write run.
- Backfill existing DB-backed Notion sources through the new command.
- Update roadmap/spec docs so this is not confused with archived-source
  retirement.

## Non-Goals

- Do not put raw source full text into mempalace drawers in this PR.
- Do not retire archived Notion records.
- Do not run LLM summarization or create summary pages.
- Do not change `write_projection` ownership; it still owns only `pages/`,
  `index.md`, and `log.md`.

## Success Criteria

- Dry-run reports the 176 missing production source Markdown files.
- Apply writes `sources/x/*.md` and `sources/wechat/*.md` with standard
  frontmatter.
- Re-running apply is idempotent.
- Future `notion-sync --sync-wiki` writes visible source Markdown for new
  records.
