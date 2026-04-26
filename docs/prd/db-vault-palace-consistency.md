# PRD: DB/Vault/Palace Consistency Governance

## Summary

- Goal: make `wiki.db` the visible source of truth for consistency fixes, then
  regenerate Obsidian Vault and Mempalace from repeatable program paths.
- User value: the user can point at problems in Obsidian, while the system fixes
  the real source layer instead of hand-editing generated Markdown.
- Success criteria: a timestamped audit compares DB, Vault, and Mempalace; a
  validated plan proposes only whitelisted fixes; dry-run is default; apply
  updates DB first, regenerates Vault projection, and repairs Mempalace page
  mirrors without direct SQL edits to `palace.db`.

## Problem

The current Vault graph exposes symptoms that are easy to see but not always
safe to fix directly:

- Some Vault files are empty or unmanaged.
- Some page Markdown still links to old Notion export filenames.
- Many compiled sources are still graph orphans because the DB/page layer does
  not expose a deterministic source-to-summary link.
- Mempalace is invisible to the user, so page mirror drift can go unnoticed.

The system model is:

1. `wiki.db` is the canonical state.
2. Obsidian Vault is a projection.
3. `palace.db` is a derived search/memory layer.

Therefore fixes must flow from DB/program paths to projections, not from manual
Vault edits back into truth.

## Scope

In:

- New consistency audit covering DB, Vault, and Mempalace.
- Timestamped JSON/Markdown reports under the vault `reports/` directory.
- Deterministic plan generation from audit evidence.
- Whitelist-only dry-run/apply execution.
- DB-first fixes for source-summary reference links when exact evidence exists;
  stale Notion links are reported for human review unless target intent is safe.
- Vault cleanup only for DB-unmanaged empty files or approved report garbage.
- Mempalace page mirror audit and repair through existing sink/replay paths.

Out:

- Full source-body import into Mempalace.
- LLM writing files or SQLite rows directly.
- Direct manual edits to `palace.db`.
- Running `batch-ingest`.
- Fuzzy source-summary auto-linking without exact URL/title evidence.

## Product Decisions

- The user judges symptoms from Obsidian, but the implementation must inspect
  `wiki.db` before deciding the fix.
- JSON reports are machine truth; Markdown reports are Chinese human views.
- `consistency-apply` defaults to dry-run and requires `--apply` to mutate.
- Mempalace source drawers remain out of scope for this PR; only page mirrors
  are audited and repaired.
- Exact source-summary links can be fixed automatically only when both sides can
  be matched by stable DB/Vault evidence.

## Acceptance

- `consistency-audit` reads all three layers when paths are provided.
- Audit reports are timestamped and do not create latest pointer files.
- `consistency-plan` rejects actions outside audit evidence and whitelist.
- `consistency-apply` dry-run writes nothing.
- `consistency-apply --apply` mutates in this order: DB, Vault projection,
  Mempalace page replay.
- Apply never directly edits `palace.db` tables.
- Apply never deletes DB-known page/source files.
- Apply never runs `batch-ingest`.
- Tests cover DB/Vault/Palace drift, source-summary exact matching, dry-run,
  whitelist enforcement, and replay safety.

## Status

- [x] PRD drafted from approved implementation plan
- [x] Spec trio complete
- [x] Implementation complete
- [ ] Review complete
- [ ] CI green
- [ ] Merged
