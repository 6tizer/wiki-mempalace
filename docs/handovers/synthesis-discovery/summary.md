# Handoff: Synthesis Discovery

Implementation complete and merged as PR #105.

PR5 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3.

## Scope

- Added typed synthesis discovery contracts.
- Extended governance scan synthesis signals with existing synthesis topic
  coverage.
- Added `research-synthesis discover`.
- Implemented single/double/triple/quad candidate discovery, ranking, dedupe,
  and per-run caps.
- Kept discovery read-only; PR6 owns web-backed composition.

## Main Files

- `crates/wiki-core/src/synthesis_discovery.rs`
- `crates/wiki-core/src/governance.rs`
- `crates/wiki-kernel/src/synthesis_discovery.rs`
- `crates/wiki-kernel/src/governance_scan.rs`
- `crates/wiki-cli/src/main.rs`
- `crates/wiki-cli/src/commands/dispatch.rs`
- `crates/wiki-cli/src/governance.rs`
- `crates/wiki-cli/tests/research_synthesis_discovery.rs`

## Verification

Focused tests passed locally:

- `cargo test -p wiki-core synthesis_discovery`
- `cargo test -p wiki-kernel synthesis_discovery`
- `cargo test -p wiki-cli --test research_synthesis_discovery`

Full local gates passed:

- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
