# Design: Audit Disposition PR2 Doc Consistency

## Summary

The repo already uses workspace edition 2021. Active docs still contained a
stale claim that `rust-mempalace` independently used a newer edition. This PR
updates active docs only and keeps archived historical text unchanged.

## Plain-Language Design

- Module role: make current docs match current code.
- Data it asks for: `Cargo.toml`, active docs, audit disposition plan.
- Data it returns: corrected active documentation.

## Current Facts

- Root `Cargo.toml` defines workspace package `edition = "2021"`.
- `crates/rust-mempalace/Cargo.toml` uses `edition.workspace = true`.
- Notion Incremental Sync has completed through PR #36/#38/#42.
- Archived Source Retirement has completed through PR #68/#69.

## Edit Plan

- `docs/architecture.md`: update architecture debt line to workspace edition 2021 inheritance.
- `crates/rust-mempalace/README.md`: update environment requirement to edition 2021 inheritance.
- `docs/roadmap.md`: mark PR1 complete, PR2 active, and avoid wording that itself looks like a stale Notion sync claim.
- `docs/specs/README.md` and PRD: record PR2 state.
- `docs/LESSONS.md`: add concise lesson for audit disposition docs consistency.

## Compatibility

- Docs-only change.
- No CLI/MCP/Cargo behavior changes.
- Archived docs remain historical and are not treated as current truth.

## Test Strategy

- `rg` active docs for old edition 2024 wording.
- `rg` active docs for stale Notion sync not-implemented wording.
- Standard repo gates still run for workflow consistency.
