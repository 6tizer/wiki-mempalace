# Consistency Plan/Apply Handoff

## Scope

- Task: consistency plan schema, validation, dry-run/apply executor.
- Owner files:
  - `crates/wiki-cli/src/consistency.rs`
  - `crates/wiki-cli/tests/consistency.rs`
  - `crates/wiki-cli/src/main.rs`

## Implemented

- `consistency-plan`
  - Requires `consistency-audit-<timestamp>.json`.
  - Generates `consistency-plan-<timestamp>.json/.md`.
  - Markdown is Chinese.
  - Actions are limited to `db_fix`, `vault_cleanup`, `palace_replay`,
    `needs_human`, and `deferred`.
  - Plan validation rejects paths/actions outside audit evidence.

- `consistency-apply`
  - Defaults to dry-run.
  - `--apply` is required for mutation.
  - Validates full plan before first write.
  - Applies DB fixes before Vault projection.
  - Deletes only audit-listed unmanaged empty files under `pages/` or `sources/`.
  - Replays Mempalace pages through `LiveMempalaceSink::on_page_written`.
  - Does not direct-write `palace.db`.

## Limits

- Stale Notion links are reported as `needs_human`; automatic rewrite is not
  safe without target intent.
- Source bodies are still not inserted into Mempalace.
- Mempalace replay currently repairs page drawers only.

## Verification

```bash
cargo test -p wiki-cli --test consistency
```

Result: 10 passed.
