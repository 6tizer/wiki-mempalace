# PRD: Vault Backfill and Palace Init

## Summary

- Goal: make the existing Obsidian vault the bootstrap source for a real
  `wiki.db` and derived `palace.db`.
- User value: multiple MCP-connected agents can share the same historical
  knowledge base instead of querying an empty runtime store.
- Success criteria: vault audit is reproducible, historical sources/pages have
  stable IDs, vault content is idempotently backfilled into `wiki.db` with
  outbox, `palace.db` can be rebuilt from outbox, and daily agent commands use
  vault-local `.wiki/` databases by default.

## Problem

The vault at `/Users/mac-mini/Documents/wiki` already contains 1000+ source
files and 3000+ page files, but the current repo-root `wiki.db` is effectively
empty. Most sources are marked `compiled_to_wiki: true`, so normal
`batch-ingest` skips the historical corpus. This is not a mempalace consumer lag
issue. The missing layer is a repeatable backfill path from existing vault
Markdown into `wiki.db` and outbox, then into `palace.db`.

Current verified facts:

- Vault total: 4496 files, 4484 Markdown files.
- Sources: 1099 Markdown files; 1082 have `compiled_to_wiki: true`, 1 has
  `compiled_to_wiki: false`, 16 are missing `compiled_to_wiki`.
- Pages: 3376 Markdown files across summary, concept, entity, synthesis, qa,
  index, and lint-report entry types.
- Current repo-root `wiki.db`: empty sources/claims/pages/entities/edges,
  one `query_served` outbox event, zero embeddings.
- No `palace.db` was found under the checked repo/vault paths.
- Current vault frontmatter has `notion_uuid` on sources/pages, but no
  `source_id`, `page_id`, or `wiki_id`.

## Product Decisions

- Mempalace consumes high-quality knowledge, not raw source sprawl by default:
  summary/concept/entity/synthesis/qa pages and structured claims are eligible;
  full source bodies are not imported into mempalace unless explicitly marked in
  a later workflow.
- Backfill must add stable IDs to the vault: source files get `source_id`, page
  files get `page_id`. `notion_uuid` remains provenance, not the system primary
  key. Relative paths remain file location, not identity.
- Historical backfill and MCP-connected agents default to `shared:wiki`, because
  the target setup is a shared knowledge base across multiple terminals and
  agents.
- Orphan source governance is a follow-up module after a fresh audit. Old
  orphan reports are not treated as current truth.
- All import/backfill operations must be idempotent, support dry-run, support
  limit, and write reports.
- No direct manual edits to SQLite databases. All state changes must go through
  repeatable code paths.

## Scope

In:

- B1 vault-audit: read-only audit of vault structure, frontmatter,
  `entry_type`, `compiled_to_wiki`, stable ID coverage, orphan/backfill
  eligibility, and JSON/Markdown report outputs.
- B2 vault-backfill: idempotent import of existing sources/pages into formal
  `wiki.db` using stable IDs and `shared:wiki`, generating outbox suitable for
  downstream consumers.
- B3 palace-init: rebuild `palace.db` from `wiki.db` outbox and verify drawers,
  kg facts, query, explain, and mempalace fusion.
- B4 agent-runtime-defaults: fix and document normal operation paths:
  - vault: `/Users/mac-mini/Documents/wiki`
  - wiki db: `/Users/mac-mini/Documents/wiki/.wiki/wiki.db`
  - palace db: `/Users/mac-mini/Documents/wiki/.wiki/palace.db`
  - default viewer/write scope: `shared:wiki`
- B5 orphan-governance: use the 2026-04-25 production B1 audit report to
  produce a read-only governance report that classifies orphan candidates,
  unsupported/missing frontmatter, pages missing `status`, and sources missing
  `compiled_to_wiki` into report-only, future auto-fix, and human-required
  lanes.

Out:

- No default LLM rerun for all historical sources.
- No forced reverse-engineering of complex claims from existing Markdown in v1.
- No default full-source-body import into mempalace.
- No automatic orphan source rewrite before B1-B4 are working.
- No web server or UI-heavy dashboard work.
- No PRD scope expansion without user approval.

## Modules

| Module | Goal | Owner area | Status |
| --- | --- | --- | --- |
| B1 vault-audit | Produce read-only JSON + Markdown reports under `reports/` with source/page/frontmatter/backfill readiness stats | `wiki-cli`, audit/report docs | Implemented, focused review addressed |
| B2 vault-backfill | Add stable IDs and import vault sources/pages into `wiki.db` with idempotency, dry-run, limit, reports, and outbox | `wiki-core`, `wiki-kernel`, `wiki-storage`, `wiki-cli` | Implemented, focused review addressed |
| B3 palace-init | Consume generated outbox into `palace.db` and validate mempalace search/fusion paths | `wiki-mempalace-bridge`, `wiki-cli`, `rust-mempalace` | Implemented, focused review addressed |
| B4 agent-runtime-defaults | Make shared vault-local paths and `shared:wiki` usable by normal CLI/MCP agent workflows | `wiki-cli`, docs, templates | Implemented, focused review addressed |
| B5 orphan-governance | Classify fresh production audit findings into safe governance lanes and run whitelist apply | `wiki-cli`, audit/report docs | Merged PR #28 / PR #30 |

## Acceptance

- Audit command is read-only against `/Users/mac-mini/Documents/wiki`.
- Audit report counts sources, pages, frontmatter fields, entry types,
  `compiled_to_wiki`, ID coverage, duplicate identity candidates, and orphan
  categories.
- Backfill supports `--dry-run`, `--limit`, report output, and repeated runs
  without duplicate logical records.
- Backfill can add missing `source_id` / `page_id` through a controlled,
  reportable, idempotent path.
- Backfill creates or updates `/Users/mac-mini/Documents/wiki/.wiki/wiki.db`
  through repository code paths, not manual SQL.
- Backfill imports existing page content as `WikiPage` records and existing
  source records as `RawArtifact` records.
- Backfill emits outbox events suitable for mempalace consumption.
- Palace init creates `/Users/mac-mini/Documents/wiki/.wiki/palace.db` from
  `wiki.db` outbox.
- Query/explain with `--palace-db` returns wiki plus mempalace fusion candidates
  where available.
- MCP-connected agents can read/write the shared corpus through `shared:wiki`.
- Agent runbook gives exact default commands for audit, backfill dry-run,
  backfill apply, palace init, query, explain, metrics, and dashboard.
- Orphan governance produces JSON as source of truth plus Markdown sibling
  report under the vault reports directory.
- Orphan governance v1 is read-only: no source/page rewrite, no
  `compiled_to_wiki` flip, no `status` insertion, no archive moves.
- Workspace checks pass before PR merge.

## Risks

- Markdown frontmatter drift from the current schema.
- Duplicate source/page identity from title, path, and Notion provenance
  mismatch.
- Adding IDs to historical files creates a large vault diff if not isolated and
  reviewed carefully.
- Outbox replay can duplicate palace records unless stable keys are used.
- Importing full source bodies into palace can bloat retrieval and reduce
  quality; v1 avoids this by default.
- Current scope defaults in CLI/MCP are not aligned with the desired multi-agent
  shared setup and may require explicit default-path/default-scope changes.
- Existing orphan audit may be stale; fresh B1 output must drive B5.
- The 2026-04-25 audit does not list every path for missing `status` and
  missing `compiled_to_wiki`; B5 must classify counts conservatively and avoid
  apply behavior until path-level evidence exists.

## Rollout

- Branch strategy: use `codex/vault-backfill-palace-init`; do not develop on
  `main`.
- Process: PRD approval -> plain architecture approval -> spec trio per module
  -> Plan mode -> task grading -> subagent split -> focused review ->
  integration review -> draft PR -> CI -> merge.
- Subagent split should keep owner files disjoint:
  - B1 audit/reporting.
  - B2 identity/backfill/storage.
  - B3 palace consume/fusion validation.
  - B4 runtime defaults/docs.
- B5 starts only after B1 produces a current orphan report. Its first version is
  report-only; future apply behavior needs a separate user approval gate.

## Status

- [x] Initial repo/vault/db facts verified
- [x] Product decisions discussed
- [x] PRD drafted
- [x] PRD approved
- [x] Plain architecture approved
- [x] Specs created
- [x] Plan mode complete
- [x] Modules implemented
- [x] Focused reviews complete
- [x] Integration review complete
- [x] Draft PR opened
- [x] CI green
- [x] Merged
- [x] Roadmap and lessons updated
- [x] B5 orphan governance complete
