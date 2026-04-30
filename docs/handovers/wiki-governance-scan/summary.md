# Handoff: Wiki Governance Scan

## Scope

PR2 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3. Merged
as PR #102.

Implemented a read-only `wiki-cli governance scan` that produces typed JSON and
rendered Markdown for downstream Fixer/Synthesis work.

## Files

- `crates/wiki-core/src/governance.rs`
- `crates/wiki-kernel/src/governance_scan.rs`
- `crates/wiki-cli/src/governance.rs`
- `crates/wiki-cli/src/main.rs`
- `docs/specs/wiki-governance-scan/*`

## Guarantees

- Does not call engine write APIs.
- Does not write projection or palace DB.
- Uses viewer scope for source/page/claim visibility.
- JSON report is the source of truth.

## Verification

- `cargo test -p wiki-core governance -- --nocapture`
- `cargo test -p wiki-kernel governance_scan -- --nocapture`
- `cargo test -p wiki-cli --bin wiki-cli governance -- --nocapture`
- `cargo test -p wiki-cli --test governance -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`

All local gates passed before PR. GitHub quick passed before merge.
