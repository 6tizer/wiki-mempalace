# PRD: Dependency Audit Automation

## Status

Implemented in PR #66. Audit v2 PR 07 将 required PR gate 扩展到
`cargo-deny`；本 PRD 的 scheduled/manual 边界仍只约束 `cargo audit`
重型 artifact lane。

## Goal

Add a low-frequency supply-chain audit lane for Rust dependencies without slowing the required quick PR path.

## Scope

- Scheduled and manually runnable GitHub Actions workflow.
- `cargo audit` wrapper script.
- JSON/stderr artifact upload for each run.
- Quick CI syntax check for the wrapper only.

## Out of Scope

- Making `cargo audit` artifact lane a required pull-request check.
- Remediating any advisory found by the audit.
- Adding new runtime dependencies.
- Writing audit output into the production Vault report tree.

## Success Criteria

- Dependency audit can run from `workflow_dispatch`.
- Dependency audit runs weekly on schedule.
- The workflow installs `cargo-audit`, runs `cargo audit --deny warnings --json`, and uploads artifacts.
- Quick CI remains unchanged for Rust test scope and only syntax-checks the wrapper script.
