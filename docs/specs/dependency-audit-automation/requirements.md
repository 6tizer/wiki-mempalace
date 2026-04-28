# Requirements: Dependency Audit Automation

## Functional Requirements

- The repository MUST provide a dependency audit workflow that can be run manually.
- The workflow MUST run on a low-frequency schedule.
- The workflow MUST not run on `pull_request`.
- The audit MUST use `cargo audit` or an equivalent RustSec-backed check.
- The audit MUST fail the workflow on warnings/advisories reported by the tool.
- The audit MUST upload machine-readable output as an artifact when possible.

## Non-Functional Requirements

- Quick CI MUST not install or run `cargo-audit`.
- The wrapper script MUST be shell-syntax checked by quick CI.
- The implementation MUST not add a new Rust crate dependency.

## Acceptance

- `bash -n scripts/dependency-audit.sh`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
