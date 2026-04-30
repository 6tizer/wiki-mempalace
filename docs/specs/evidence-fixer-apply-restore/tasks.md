# Tasks: Evidence Fixer Apply + Restore

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| PRD/spec setup | Main | `docs/prd/`, `docs/specs/evidence-fixer-apply-restore/` | Complete |
| Apply/restore contracts | Main | `crates/wiki-core/src/evidence_fixer.rs` | Complete |
| Engine mutation helpers | Main | `crates/wiki-kernel/src/engine.rs` | Complete |
| Executor | Main | `crates/wiki-kernel/src/evidence_fixer_apply.rs` | Complete |
| CLI commands + reports | Main | `crates/wiki-cli/src/main.rs`, `crates/wiki-cli/src/governance.rs` | Complete |
| Restore and reliability tests | Main | kernel + CLI tests | Complete |
| Focused review + gates | Main | local commands / GitHub CI | Complete; merged PR #104 |

## Verification

- `cargo test -p wiki-core evidence_fixer -- --nocapture`
- `cargo test -p wiki-kernel evidence_fixer_apply -- --nocapture`
- `cargo test -p wiki-cli --test governance_fixer_apply -- --nocapture`
- `cargo test -p wiki-cli --bin wiki-cli governance -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`

All local gates and GitHub `quick` passed before merge in PR #104.
