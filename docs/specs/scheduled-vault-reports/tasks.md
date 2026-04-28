# Tasks: Scheduled Vault Reports

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/scheduled-vault-reports`
- [x] Add `vault-reports` automation job
- [x] Add timestamped report bundle writer
- [x] Add latest pointers
- [x] Add retention pruning
- [x] Add focused tests
- [x] Handoff
- [x] Local gate
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Automation registry job | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Report bundle writer | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Retention + latest pointers | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |
| CLI integration tests | Agent | Main | `crates/wiki-cli/tests/automation_run_daily.rs` | Complete |
| Backfill docs/roadmap/LESSONS | Script | Main | `docs/` | Complete |

## Verification

- `cargo test -p wiki-cli scheduled_report -- --nocapture`
- `cargo test -p wiki-cli --test automation_run_daily vault_reports -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- PR #67 merged on 2026-04-28.
