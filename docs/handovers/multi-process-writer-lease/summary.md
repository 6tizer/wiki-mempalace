# Handoff: Multi-process Writer Lease

## Branch

`codex/multi-process-writer-lease`

## Status

PR #64 merged on 2026-04-28.

## Completed

- Add repository-adjacent writer lease file.
- Acquire lease for CLI/MCP writer entrypoints before engine load.
- Keep read-only/report commands unblocked.
- Support stale lease replacement by TTL.
- Keep SQLite transaction boundaries unchanged.

## Verification

- `cargo test -p wiki-storage writer_lease -- --nocapture` passed.
- `cargo test -p wiki-cli writer_lease -- --nocapture` passed.
- `cargo test -p wiki-cli --test writer_lease -- --nocapture` passed.
- `cargo test --workspace` passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
