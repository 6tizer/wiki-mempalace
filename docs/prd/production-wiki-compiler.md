# PRD: Production Wiki Compiler

**Status**: implementation merged in PR #44; production tiny sample pending
**Related**: `Notion Incremental Sync`, `Notion Source Vault Projection`, `Vault Backfill + Palace Init`, `DB/Vault/Palace Consistency Governance`

## Goal

Implement and run the production Wiki Compiler on real content:

Notion/raw source in `wiki.db` -> Wiki Compiler -> summary + concept/entity
pages -> Vault projection -> outbox -> Mempalace -> query/explain ->
human-readable reports.

## Current State

- `/Users/mac-mini/Documents/wiki/.wiki/wiki.db` is the source of truth.
- `/Users/mac-mini/Documents/wiki` is the Obsidian projection layer.
- `/Users/mac-mini/Documents/wiki/.wiki/palace.db` is the Mempalace projection layer.
- 176 DB-backed Notion sources exist and are visible under `sources/x` and
  `sources/wechat`.
- All 176 DB-backed Notion sources are still raw sources; they have not been
  compiled into wiki pages.
- Mempalace currently has page drawers from historical backfill, but the Notion
  source compilation path has not been exercised as a full production loop.

## Target Business Flow

For each real article, the compiler should normally produce:

- 1 summary page:
  - title: `摘要：<article title>`
  - entry_type: `summary`
  - status: approved
  - body: one-sentence summary, 3-5 key insights, extracted concepts, original
    article info, personal note
- 3-7 concept/entity updates:
  - create a new concept page when a key idea does not exist
  - create an entity page when the item is a concrete product, company, person,
    or project
  - update an existing concept/entity when it already exists
  - append a deduplicated source reference back to the summary
- Source writeback:
  - after a successful compile, mark the original source as compiled
  - Notion writeback remains disabled for the first production sample unless
    explicitly enabled later

The important distinction: claims/entities/relations stored only inside
`wiki.db` are not enough for the target user workflow. Concepts and entities
must become visible wiki entries when they are meaningful.

## Problem

The system has many pieces implemented and individually verified, but the actual
compiler contract still does not match the user's Notion Wiki Compiler
Instructions. The current local
`batch-ingest` path mainly materializes a summary page and stores claims /
entities / relations structurally; it does not fully match the Notion compiler
workflow that creates or updates visible concept/entity pages with backlink
references. This PRD upgrades the compiler contract first, then proves it with a
tiny production run.

## Scope

### Phase 0: Read-only Baseline

- Confirm DB/Vault/Palace paths.
- Count uncompiled Notion sources.
- Confirm outbox consumer progress is at head.
- Confirm query and query/explain work before new writes.
- Record baseline counts for sources, pages, outbox, palace drawers/facts, and
  reports.

### Phase 1: Compiler Contract

- Implement the Notion-equivalent compiler contract:
  - summary creation
  - concept/entity extraction
  - concept/entity create-or-update with deduplication
  - summary -> concept/entity links
  - concept/entity -> summary source references
  - local source compiled marker
- Keep existing automation-compatible `batch-ingest` behavior, but make it use
  the new compiler contract internally.

### Phase 2: Tiny Production Sample

- Back up the production vault before writes.
- Select a very small sample:
  - 1 X source.
  - 1 WeChat source.
- Compile only those sources through the target compiler path.
- Use existing CLI/LLM paths; do not hand-edit generated Markdown.

### Phase 3: Layer Verification

- Verify DB source state changed from raw/uncompiled to compiled.
- Verify the summary page was written under `pages/summary/`.
- Verify concept/entity pages were created or updated under their managed page
  directories when the article contains meaningful concepts/entities.
- Verify summary -> concept/entity references and concept/entity -> summary
  source references.
- Verify source/page links and frontmatter.
- Verify outbox contains expected `PageWritten` events.
- Consume outbox to Mempalace.
- Verify Mempalace receives the new page drawers.
- Run `query` and `query/explain --palace-db` against the compiled topic.

### Phase 4: Human Check

- User checks Obsidian:
  - source file remains readable.
  - generated page exists.
  - page title/body are useful.
  - links make sense.
- If the tiny sample is bad, stop and improve compilation before scaling.

### Phase 5: Scale Decision

- If sample passes, choose one of:
  - compile 5 more sources,
  - compile one full origin lane (`x` or `wechat`),
  - schedule a controlled batch job,
  - pause and improve prompt/schema first.

## Non-Goals

- Do not compile all 176 Notion sources in the first run.
- Do not retire archived Notion sources in this PRD.
- Do not redesign tags, schema, or page taxonomy.
- Do not write directly to `palace.db`.
- Do not manually edit generated production Markdown as the fix path.
- Do not enable unattended automation until the sample loop is proven.

## Success Criteria

- A small real sample is compiled without DB/Vault/Palace drift.
- At least one summary page appears in Obsidian under `pages/summary/`.
- Meaningful concepts/entities from the sample are visible as wiki entries, not
  only stored as hidden DB records.
- The corresponding `PageWritten` event is consumed by Mempalace.
- `query` and `query/explain --palace-db` can surface the generated page.
- A run report records commands, counts, backup path, outputs, and known issues.
- The next scale decision is explicit and documented.

## Stop Conditions

- Backup fails.
- LLM output is malformed or low quality.
- A generated summary/concept/entity page violates `docs/vault-standards.md`.
- outbox consumer progress does not advance after consume.
- Mempalace misses an eligible generated page.
- Obsidian view is confusing enough that the user cannot trust the output.

## Open Questions

1. Sample selection: default to recent high-signal sources, unless the user
   names specific files before the production run.
2. Page type policy: create both concept and entity pages in the first compiler
   implementation, because the Notion contract already distinguishes them.
3. Writeback: mark the local source as compiled; keep Notion writeback disabled
   for the first production sample.
