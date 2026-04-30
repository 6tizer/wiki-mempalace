# Next Three Closeout 2026-04-30 Tasks

## Implementation

- [x] Verify PR #22 is merged.
- [x] Delete stale remote branch `codex/vault-report-paths`.
- [x] Add `SqliteRepository::open_read_only`.
- [x] Add no-engine `wiki-cli verify-row-state`.
- [x] Report legacy blob-only DBs as clear validation failures.
- [x] Add focused CLI tests for row-state verification.
- [x] Run read-only production validation.
- [x] Observe scheduled hardening workflow.
- [x] Update roadmap/spec/PRD/handoff/lessons docs.

## Validation

- [x] `cargo test -p wiki-cli --test row_state_verify -- --nocapture`
- [x] `cargo test -p wiki-storage row_level_state -- --nocapture`
- [x] `git diff --check`
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo deny --all-features check advisories bans licenses sources`

## Review Notes

- Production blob fallback is not ready to retire because live `wiki.db` has `rows=0` for row-level state.
- Hardening scheduled lane is green; no follow-up fix PR is needed for this observation.
