# Design: Dependency Audit Automation

## Workflow

`.github/workflows/dependency-audit.yml` is independent from quick CI:

- triggers: `workflow_dispatch` and weekly schedule;
- install: `cargo install cargo-audit --locked` when missing;
- run: `scripts/dependency-audit.sh`;
- output: `artifacts/dependency-audit/dependency-audit.json` and stderr text;
- retention: 30 days.

## Wrapper

`scripts/dependency-audit.sh` owns local/report behavior:

- fails fast if `cargo-audit` is missing;
- creates the artifact directory;
- runs `cargo audit --deny warnings --json`;
- writes a short GitHub step summary when available;
- exits with the original `cargo audit` status.

## CI Boundary

`ci-quick.yml` only adds `bash -n scripts/dependency-audit.sh`. It does not install `cargo-audit`, fetch the advisory DB, or run the audit on PRs.
