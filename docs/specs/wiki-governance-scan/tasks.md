# Tasks: Wiki Governance Scan

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| PRD/spec setup | Main | `docs/prd/`, `docs/specs/wiki-governance-scan/` | Complete |
| Report contract | Main | `crates/wiki-core/src/governance.rs` | Complete |
| Pure scanner | Main | `crates/wiki-kernel/src/governance_scan.rs` | Complete |
| CLI command + report writing | Main | `crates/wiki-cli/src/main.rs`, `crates/wiki-cli/src/governance.rs` | Complete |
| Scope/read-only tests | Main | kernel + CLI tests | Complete |
| Focused review + gates | Main | local commands / GitHub CI | Complete; merged PR #102 |

## Verification

- `cargo test -p wiki-core governance -- --nocapture`
- `cargo test -p wiki-kernel governance_scan -- --nocapture`
- `cargo test -p wiki-cli --bin wiki-cli governance -- --nocapture`
- `cargo run -p wiki-cli -- --db <temp.db> --viewer-scope private:cli governance scan --json`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
