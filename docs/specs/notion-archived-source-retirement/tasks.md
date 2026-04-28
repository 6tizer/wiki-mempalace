# Tasks: Notion Archived Source Retirement

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/notion-archived-retirement-plan`
- [x] Storage API lists `notion_page_index`
- [x] Notion client retrieves page archive state
- [x] Dry-run plan report model
- [x] CLI command wired
- [x] Apply command
- [x] DB source + Notion index retirement
- [x] Vault source file cleanup
- [x] Focused tests
- [x] Handoff
- [x] Local full gate
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Storage index listing | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Notion archive state client | Agent | Main | `crates/wiki-cli/src/notion_client.rs` | Complete |
| Plan report builder/writer | Agent | Main | `crates/wiki-cli/src/notion_archived_retirement.rs` | Complete |
| CLI command integration | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Apply command | Agent | Main | `crates/wiki-cli/src/main.rs` / `crates/wiki-cli/src/notion_archived_retirement.rs` | Complete |
| Atomic DB retirement | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| CLI apply regression | Agent | Main | `crates/wiki-cli/tests/vault_cli_commands.rs` | Complete |
| Docs/roadmap/handoff | Agent | Main | `docs/` | Complete |

## Verification

- `cargo test -p wiki-storage notion_page_index -- --nocapture`
- `cargo test -p wiki-cli notion_client_retrieves_page_archive_state -- --nocapture`
- `cargo test -p wiki-cli notion_archived_retirement -- --nocapture`
- `cargo test -p wiki-cli --test vault_cli_commands notion_archived_retirement_apply_defaults_to_dry_run_and_apply_retires_source -- --nocapture`
- `cargo test -p wiki-storage snapshot_and_notion_index_deletes_commit_together -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`

## Merge

- Audit/plan PR #68 merged on 2026-04-28.
- Apply PR #69 merged on 2026-04-28.
