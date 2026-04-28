# Handoff: Dependency Audit Automation

## Branch

`codex/dependency-audit-automation`

## Status

PR #66 merged on 2026-04-28.

## Completed

- Added `.github/workflows/dependency-audit.yml`.
- Added `scripts/dependency-audit.sh`.
- Added quick CI shell syntax coverage for the new script.
- Backfilled PRD/spec/tasks/roadmap/LESSONS.

## Behavior

- Runs manually through `workflow_dispatch`.
- Runs weekly at `0 3 * * 1` UTC.
- Does not run on pull requests.
- Uploads JSON/stderr artifacts under `artifacts/dependency-audit/`.

## Verification

- `bash -n scripts/dependency-audit.sh` passed.
- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
