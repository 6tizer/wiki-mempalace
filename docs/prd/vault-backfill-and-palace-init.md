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
- B5 orphan-governance planning: use B1 output to define a later safe cleanup
  path for orphan sources.

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
| B5 orphan-governance | Plan and later implement safe orphan source cleanup from fresh B1 evidence | docs/specs follow-up | Deferred until B1 report |

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
- B5 starts only after B1 produces a current orphan report and user confirms the
  cleanup policy.

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
- [ ] CI green
- [ ] Merged
- [ ] Roadmap and lessons updated
