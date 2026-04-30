# Tasks: Evidence Fixer Plan

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| PRD/spec setup | Main | `docs/prd/`, `docs/specs/evidence-fixer-plan/` | Complete |
| Plan contract | Main | `crates/wiki-core/src/evidence_fixer.rs` | Complete |
| Pure planner | Main | `crates/wiki-kernel/src/evidence_fixer_plan.rs` | Complete |
| CLI command + report writing | Main | `crates/wiki-cli/src/main.rs`, `crates/wiki-cli/src/governance.rs`, `crates/wiki-cli/src/commands/dispatch.rs` | Complete |
| Privacy/web guard tests | Main | kernel + CLI tests | Complete |
| Focused review + gates | Main | local commands / GitHub CI | Local complete; PR CI pending |

## Verification

- `cargo test -p wiki-core evidence_fixer -- --nocapture`
- `cargo test -p wiki-kernel evidence_fixer_plan -- --nocapture`
- `cargo test -p wiki-cli --test governance_fixer_plan -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`

All local gates passed before PR.
