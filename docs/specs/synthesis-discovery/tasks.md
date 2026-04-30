# Tasks: Synthesis Discovery

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| PRD/spec setup | Main | `docs/prd/`, `docs/specs/synthesis-discovery/` | Complete |
| Core report contracts | Main | `crates/wiki-core/src/synthesis_discovery.rs` | Complete |
| Governance scan coverage signal | Main | `crates/wiki-core/src/governance.rs`, `crates/wiki-kernel/src/governance_scan.rs` | Complete |
| Discovery algorithm | Main | `crates/wiki-kernel/src/synthesis_discovery.rs` | Complete |
| CLI command + report output | Main | `crates/wiki-cli/src/main.rs`, `crates/wiki-cli/src/governance.rs`, `crates/wiki-cli/src/commands/dispatch.rs` | Complete |
| Focused tests | Main | kernel + CLI tests | Complete |
| Full gates + PR | Main | local commands / GitHub CI | Complete; merged PR #105 |

## Verification

- `cargo test -p wiki-core synthesis_discovery`
- `cargo test -p wiki-kernel synthesis_discovery`
- `cargo test -p wiki-cli --test research_synthesis_discovery`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`

All local gates and GitHub `quick` passed before merge in PR #105.
