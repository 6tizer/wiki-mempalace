# Module Handoff: Persist Snapshot + Outbox

## Summary

- Implemented C16A atomic persistence for engine snapshot + current outbox.
- Production save+flush call sites now use one durability boundary instead of
  separate snapshot and outbox commits.

## Files Changed

| File | Change | Reason |
| --- | --- | --- |
| `crates/wiki-storage/src/lib.rs` | Added `WikiRepository::save_snapshot_and_append_outbox`; SQLite implementation wraps snapshot + events in `BEGIN IMMEDIATE` / `COMMIT`; rollback test added | Prevent partial snapshot/outbox commits |
| `crates/wiki-kernel/src/engine.rs` | Added `save_to_repo_and_flush_outbox_with_policy`; clears in-memory outbox only after successful atomic commit | Central engine write path |
| `crates/wiki-cli/src/main.rs` | Replaced CLI write paths with atomic engine commit | Remove production save-then-flush pattern |
| `crates/wiki-cli/src/mcp.rs` | Replaced MCP `save_and_flush` helper with atomic engine commit | Remove MCP save-then-flush pattern |
| `crates/wiki-cli/src/vault_backfill.rs` | Uses atomic snapshot+page-event persistence | Keep backfill state and PageWritten events aligned |
| `docs/specs/persist-snapshot-outbox/` | Locked design option A and updated task state | Keep spec current |
| `AGENTS.md`, `docs/outbox-and-consumers.md` | Documented new durability unit | Future Agent behavior |

## Public Interfaces

- New trait method:
  - `WikiRepository::save_snapshot_and_append_outbox(&self, snapshot, events) -> Result<usize, StorageError>`
- New engine methods:
  - `save_to_repo_and_flush_outbox`
  - `save_to_repo_and_flush_outbox_with_policy`

## Known Limits

- Legacy `save_to_repo` and `flush_outbox_to_repo_with_policy` remain for
  snapshot-only or explicit outbox-only use.
- The atomic engine method writes the full current in-memory outbox in one
  transaction; if very large batches become a problem, a future spec must define
  per-chunk atomicity.
- C16B ANN embedding index remains separate future work.

## Verification

- `cargo test -p wiki-storage --quiet`
- `cargo test -p wiki-kernel --quiet`
- `cargo test -p wiki-cli --test vault_backfill --quiet`
- `cargo test -p wiki-cli --test automation_run_daily --quiet`
- `cargo test -p wiki-cli --quiet`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Next Notes

- Open PR and watch CI.
- Keep C16B ANN work separate from this storage correctness PR.
