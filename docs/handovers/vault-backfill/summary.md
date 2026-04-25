# Vault Backfill B2 Handoff

## Scope

- Owner: Subagent B.
- Files touched:
  - `crates/wiki-cli/src/vault_backfill.rs`
  - `crates/wiki-cli/tests/vault_backfill.rs`
  - `docs/handovers/vault-backfill/summary.md`
- `main.rs` not touched. CLI wiring remains for main agent.

## Implemented

- `BackfillMode::{DryRun, Apply}`.
- `VaultBackfillOptions` path-based entrypoint:
  - `backfill_vault(options)`
- Scope-string entrypoint:
  - `backfill_vault_with_scope_str(..., "shared:wiki", ...)`
- Repo-based entrypoint:
  - `backfill_vault_with_repo(vault_path, repo, scope, mode, limit, report_dir)`
- Stable UUID v5 generation for missing IDs:
  - namespace input includes vault-relative path, `notion_uuid`, and source/page kind.
  - uses local SHA-1 implementation to avoid changing Cargo features.
- Dry-run:
  - scans vault.
  - writes JSON/Markdown report only.
  - report includes planned source/page IDs.
  - does not create or mutate DB.
  - does not mutate vault Markdown.
- Apply:
  - writes missing `source_id` / `page_id` into frontmatter.
  - preserves existing IDs and existing frontmatter fields.
  - imports sources as `RawArtifact`.
  - imports pages as `WikiPage`.
  - updates existing source/page records when a matching ID exists but metadata,
    scope, entry type, status, URI/tags, or markdown drifted.
  - emits `PageWritten` outbox only once per logical page ID.
  - repairs missing `PageWritten` when a prior apply reached DB save but not
    outbox append.
  - rerun is idempotent for frontmatter, DB records, and outbox count.

## Report Files

- `vault-backfill-report.json`
- `vault-backfill-report.md`

Both are written under `report_dir`.

## Tests

Run:

```bash
cargo test -p wiki-cli --test vault_backfill
```

Covered:

- dry-run no vault mutation and no DB creation.
- apply writes missing IDs.
- existing `notion_uuid` is preserved.
- source/page records appear in `wiki.db`.
- page import emits `PageWritten`.
- rerun does not duplicate records or outbox events.
- interrupted apply repair: existing DB page with missing `PageWritten` gets one
  event on rerun, then remains idempotent.
- duplicate page IDs do not emit duplicate `PageWritten`.
- duplicate source/page IDs are skipped and reported instead of merged into one
  DB record.
- rerun repairs stale existing DB records with the same ID.
- scope string parsing.
- repo-based entrypoint.
- UUID v5 repeatability and SHA-1 known vector.

## Notes For Main Agent

- Main-agent integration added `mod vault_backfill;` and the `vault-backfill`
  CLI command.
- `PageWritten` behavior is local to backfill; existing normal page-writing flows remain unchanged.
- Source import intentionally does not emit `SourceIngested`; this keeps historical source bodies from becoming palace default content through the existing bridge path.

## Review Follow-up

- RB P2 duplicate `PageWritten` risk fixed by updating the in-run event ID set.
- RB P2 existing-record drift fixed by updating existing records through
  `WikiRepository::save_snapshot`.
- Integration P2 duplicate ID collapse fixed by rejecting duplicate
  `source_id` / `page_id` during planning.
