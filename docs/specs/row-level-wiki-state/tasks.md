# Tasks: Row-level Wiki State migration/dual-write

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Branch: `codex/row-level-wiki-state-dual-write`
- [x] Add `wiki_state_row` schema
- [x] Dual-write row-level snapshot state
- [x] Add blob-missing fallback load from rows
- [x] Add storage rollback and stale-row tests
- [x] Handoff
- [x] PR + CI green
- [x] PRD / roadmap updated

## Verification

- `cargo test -p wiki-storage row_level_state -- --nocapture`
- `cargo test -p wiki-storage`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
- production DB read-only duplicate-id check: no duplicates in source/claim/page/entity/audit ids
