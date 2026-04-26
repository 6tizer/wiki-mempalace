# PRD: Notion Source Vault Projection

**Status**: merged; production apply completed
**Related**: `Notion Incremental Sync`, `Vault Backfill + Palace Init`

## Goal

Make Notion API synced sources visible in the Obsidian vault under
`sources/{origin}/`, matching the existing vault standards.

## Problem

Before PR #42, `notion-sync` wrote `notion://` records into `wiki.db` and emitted
outbox events, but DB-backed Notion sources were not visible as source Markdown
in `/Users/mac-mini/Documents/wiki`. The first implementation also exposed two
production issues: Notion body text only contained database properties, and
raw Notion labels such as `Apache2.0` were not safe Obsidian tags.

Production evidence on 2026-04-26:

- `wiki.db` contains 176 `notion://` sources.
- `x_bookmark`: 113 sources.
- `wechat`: 63 sources.
- Final dry-run after canonical UUID de-duplication: 172 files to write, 4
  duplicate Notion UUID records already represented in the same run.

Production closeout on 2026-04-27:

- PR #42 merged.
- `notion-sync` now reads Notion page blocks and supports `--refresh-existing`.
- automation `notion-sync` refreshes existing pages in the cursor window.
- X window refresh: fetched 782, refreshed 108, skipped 674, errors 0.
- WeChat window refresh: fetched 485, refreshed 53, skipped 432, errors 0.
- `notion-source-vault-sync` applied 158 refreshed source projections.
- Final projection dry-run with `--refresh-existing --repair-tags`: planned 0,
  tags_rewritten 0.
- Validation: 176 DB-backed Notion sources, 171 bodies with at least 1000 chars,
  invalid Obsidian tags 0.

## Scope

- Add a vault projection helper for DB-backed `notion://` sources.
- Add `wiki-cli notion-source-vault-sync` with dry-run default and explicit
  `--apply`.
- Wire `notion-sync --sync-wiki` to project missing source Markdown after a
  successful write run.
- Backfill existing DB-backed Notion sources through the new command.
- Fetch Notion page blocks so source bodies contain the article text, not only
  database properties.
- Project tags in an Obsidian-safe form and provide `--repair-tags`.
- Refresh existing DB/Vault source records through `--refresh-existing`.
- Update roadmap/spec docs so this is not confused with archived-source
  retirement.

## Non-Goals

- Do not put raw source full text into mempalace drawers in this PR.
- Do not retire archived Notion records.
- Do not run LLM summarization or create summary pages.
- Do not change `write_projection` ownership; it still owns only `pages/`,
  `index.md`, and `log.md`.

## Success Criteria

- Dry-run reports 176 DB-backed Notion sources, with 172 unique production
  source Markdown files to write after duplicate UUID de-duplication.
- Apply writes `sources/x/*.md` and `sources/wechat/*.md` with standard
  frontmatter.
- Re-running apply is idempotent.
- Future `notion-sync --sync-wiki` writes visible source Markdown for new
  records.
- Future automation runs refresh existing source bodies/tags inside the cursor
  window.

## Deferred

- Raw Notion sources are not compiled wiki pages. LLM compilation into
  `pages/{summary,concept,entity,synthesis,qa}/` remains a separate workflow.
- Archived Notion source retirement remains a separate DB-first governance item.
