# PRD: Production Wiki Compiler

**Status**: implementation + deferred/fix hardening merged; production compiler scale-up fully closed
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
- Most DB-backed Notion sources are now compiled. Production scale-up ran
  controlled 10-source batches across remaining queue, and full dry-run found
  0 uncompiled sources remaining.
- Mempalace has page drawers from historical backfill. PR #46 exercised the
  source compilation path on tiny samples; PR #47 added canonicalization v2 and
  PR #54 added the deferred resolver/fixer lane needed for controlled scale-up.

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
  - Notion writeback remains disabled unless explicitly enabled later

The important distinction: claims/entities/relations stored only inside
`wiki.db` are not enough for the target user workflow. Concepts and entities
must become visible wiki entries when they are meaningful.

## Problem

The original gap was that `batch-ingest` behaved like a summary-first ingest
path instead of the user's Notion Wiki Compiler workflow. PR #44 closed that
contract gap by producing visible summary/concept/entity pages with backlinks.
PR #47 and PR #54 then added the resolver/fixer safety layer needed to scale
without creating duplicate concept/entity clutter.

The remaining problem is operational, not architectural: continue compiling the
remaining source queue in small batches while proving that each batch preserves
DB/Vault/Palace consistency and Vault content quality.

## Follow-Up: Canonicalization v2

Status: completed in PR #47 and hardened by PR #54.

The first production samples showed that prompt-only dedup is not enough. The
LLM can describe the same page with several names, such as `MCP connectors`,
`MCP连接器`, or `MCP 协议`. Adding every observed alias to code or prompt does not
scale; 100+ sources would turn the compiler into a hand-maintained glossary and
would spend context before the article is even read.

The current compiler path now includes a pre-write resolver:

```text
raw source
 -> compiler draft
 -> canonical resolver
 -> small LLM only for ambiguous matches
 -> write Wiki DB
 -> project Vault
 -> consume Mempalace
 -> lint/audit after write
```

The resolver is the main guard before DB writes. It should use normalized title
keys, existing page candidates, aliases, page type checks, and a bounded top-K
candidate set from `wiki.db`. The LLM fallback should answer only the small
question: whether a draft title and an existing page are the same canonical wiki
entry. Confirmed aliases should become data, not an ever-growing prompt.

Lint/Fixer still matter, but they are post-write governance. Their fixes must
apply to `wiki.db` first, then trigger Vault projection, Mempalace sync, and a
fresh audit. They must not patch Vault Markdown or `palace.db` directly.

The current production operation loop is:

```text
batch-ingest small batch
 -> compiler-resolve-deferred --allow-create --apply
 -> Vault projection
 -> consume-to-mempalace
 -> lint/audit
 -> duplicate/broken-link/content spot checks
```

Do not switch to unattended broad compilation until multiple small batches pass
with no new duplicate groups, no new broken wikilinks, and acceptable Vault
content quality.

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

### Phase 6: Compiler Canonicalization v2

- Insert a canonical resolver between compiler draft parsing and DB writes.
- Retrieve only relevant existing concept/entity candidates per draft item; do
  not put the whole wiki into the compiler prompt.
- Add a small LLM fallback for ambiguous candidate decisions.
- Persist confirmed aliases/canonical decisions outside the prompt and outside
  hardcoded one-off code paths.
- Keep Lint/Fixer as a post-write safety net that repairs DB state and then
  re-runs projection/sync.

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
- Before any broad compile, repeated concept/entity names from the tiny samples
  are resolved through a bounded resolver path, not through an unbounded prompt
  or hardcoded alias list.

## Stop Conditions

- Backup fails.
- LLM output is malformed or low quality.
- A generated summary/concept/entity page violates `docs/vault-standards.md`.
- outbox consumer progress does not advance after consume.
- Mempalace misses an eligible generated page.
- Obsidian view is confusing enough that the user cannot trust the output.
- Canonicalization needs a whole-wiki prompt or a large hardcoded alias list to
  pass a sample.

## Resolved Decisions

1. Sample selection started with controlled X/WeChat samples and then moved to
   10-source batches.
2. Page type policy creates both concept and entity pages because the Notion
   contract distinguishes them.
3. Local source writeback marks successful compiles as `compiled_to_wiki: true`;
   Notion writeback remains disabled unless explicitly enabled.
