# Requirements: Scheduled Vault Reports

## Functional Requirements

- The automation registry MUST include a `vault-reports` job.
- The daily automation plan MUST run `vault-reports` after content sync/consume jobs.
- The job MUST write a timestamped bundle under `<wiki-dir>/reports/scheduled/<timestamp>-scheduled/`.
- The bundle MUST include:
  - vault-audit JSON and Markdown;
  - metrics JSON and Markdown;
  - dashboard HTML;
  - automation health text;
  - M12 suggest JSON and Markdown.
- The job MUST write stable latest pointer files under `<wiki-dir>/reports/scheduled/`.
- The job MUST prune old timestamped scheduled report directories.

## Non-Functional Requirements

- The job MUST not mutate wiki content, DB pages/claims/sources, or Mempalace state.
- The job MAY write automation run status, like existing automation jobs.
- The job MUST not require network.
- Retention MUST default to a conservative value and be configurable by env var.

## Acceptance

- `cargo test -p wiki-cli scheduled_report -- --nocapture`
- `cargo test -p wiki-cli --test automation_run_daily vault_reports -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
