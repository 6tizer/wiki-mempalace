# Handoff: Evidence Fixer Apply + Restore

Implementation complete on branch `codex/evidence-fixer-apply-restore`.

PR4 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3.

## Scope

- Added `governance fixer-apply`.
- Added `governance restore`.
- Added typed apply/restore reports and tombstones.
- Added engine helpers for page delete and claim snapshot restore/delete.
- Kept Palace as downstream projection only.

## Main Files

- `crates/wiki-core/src/evidence_fixer.rs`
- `crates/wiki-kernel/src/evidence_fixer_apply.rs`
- `crates/wiki-kernel/src/engine.rs`
- `crates/wiki-cli/src/main.rs`
- `crates/wiki-cli/src/governance.rs`
- `crates/wiki-cli/tests/governance_fixer_apply.rs`

## Verification

Focused tests passed locally:

- `cargo test -p wiki-core evidence_fixer -- --nocapture`
- `cargo test -p wiki-kernel evidence_fixer_apply -- --nocapture`
- `cargo test -p wiki-cli --test governance_fixer_apply -- --nocapture`
- `cargo test -p wiki-cli --bin wiki-cli governance -- --nocapture`

Full local gates passed:

- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
