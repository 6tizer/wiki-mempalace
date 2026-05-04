# Requirements: Notion Full Reimport 2026-05-05

## Goal

Use the three latest Notion ZIP exports under
`/Users/mac-mini/wiki-migration/NotionDB导出` as the source of truth, rebuild a
fresh local wiki in staging, then replace `/Users/mac-mini/Documents/wiki` only
after staging validation passes.

## Target Counts

- Wiki pages: `4765`.
- Sources: `1526` total.
- X sources: `943`.
- WeChat sources: `583`.
- Total imported objects: `6291`.

## Behavior

- `wiki-migration-notion` must scan only official database records:
  - Wiki: direct Markdown children under `知识 Wiki/` with a recognized `类型`.
  - X: direct Markdown children under `X书签文章数据库/`.
  - WeChat: direct Markdown children under `文章数据库/`, excluding rows without
    `来源` and `文章链接`.
- Scanner skipped records must be visible in the dry-run Markdown report and
  JSONL output.
- Output filenames must be unique under case-insensitive filesystems. If two
  slugs collide by case only, append `-{uuid8}`.
- `vault-backfill --apply` must preserve page metadata from frontmatter:
  `tags`, `confidence`, `source_url`, `source_tags`, `created_at`,
  `updated_at`, and `last_compiled_at`.
- Existing DB pages must be refreshed from the latest Notion export metadata
  instead of keeping stale DB tags.
- Date parse failures must become report warnings, not hard failures.
- `notion-sync-index-backfill --apply` must run after staging DB creation so
  future Notion incremental sync does not duplicate imported X/WeChat sources.

## Boundaries

- Do not incrementally merge into the old production DB.
- Do not migrate old embeddings, old outbox state, or automation run history.
- Do not write directly to `palace.db`; rebuild palace from the fresh DB/outbox
  using the supported CLI path.
- Do not replace production until staging counts, query, lint, and consistency
  checks pass.
- All cargo commands for this work are serial. No parallel cargo.

## Acceptance

- Unit tests cover scanner filtering, case-insensitive collision handling, and
  `vault-backfill` metadata preservation/update.
- Staging `vault-backfill-report.json` has `sources_seen=1526`,
  `pages_seen=4765`, and `skipped=0`.
- Staging `wiki_state` has `sources=1526` and `pages=4765`.
- `notion_page_index` contains the X + WeChat source page indexes.
- Spot-check pages such as `QueryWeaver` keep the newest Notion tags.
- Staging and production smoke checks can query the rebuilt wiki and palace.
