# Handoff: Scheduled Vault Reports

## Branch

`codex/scheduled-vault-reports`

## Status

PR #67 merged on 2026-04-28.

## Completed

- Added `AutomationJob::VaultReports` / CLI value `vault-reports`.
- Added the job to the daily automation chain after `notion-sync`.
- Added timestamped scheduled report bundles.
- Added `latest.json` / `latest.md` pointers.
- Added retention pruning via `WIKI_SCHEDULED_REPORT_KEEP`, default `14`.

## Output

Reports are written under `<wiki-dir>/reports/scheduled/`.

## Verification

- `cargo test -p wiki-cli scheduled_report -- --nocapture` passed.
- `cargo test -p wiki-cli --test automation_run_daily vault_reports -- --nocapture` passed.
- `cargo test --workspace` passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
