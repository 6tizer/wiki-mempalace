# Handoff: Evidence Fixer Plan

Implementation complete and merged as PR #103 from branch `codex/evidence-fixer-plan`.

PR3 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3.

## Scope

- Added typed dry-run fixer plan contracts.
- Added pure planner from governance scan JSON to ready/blocked actions.
- Added `wiki-cli governance fixer-plan --scan <scan.json>`.
- Added opt-in web verification for near duplicate groups.
- Kept private-scope web search blocked unless explicitly allowed.
- Added JSON/Markdown plan reports.

## Main Files

- `crates/wiki-core/src/evidence_fixer.rs`
- `crates/wiki-kernel/src/evidence_fixer_plan.rs`
- `crates/wiki-cli/src/main.rs`
- `crates/wiki-cli/src/commands/dispatch.rs`
- `crates/wiki-cli/src/governance.rs`
- `crates/wiki-cli/tests/governance_fixer_plan.rs`

## Verification

Focused tests passed locally:

- `cargo test -p wiki-core evidence_fixer -- --nocapture`
- `cargo test -p wiki-kernel evidence_fixer_plan -- --nocapture`
- `cargo test -p wiki-cli --test governance_fixer_plan -- --nocapture`

Full local gates passed:

- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
