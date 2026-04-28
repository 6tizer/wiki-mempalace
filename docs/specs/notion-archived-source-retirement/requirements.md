# Requirements: Notion Archived Source Retirement

## Functional Requirements

- The CLI MUST provide `notion-archived-retirement plan`.
- The plan command MUST read `notion_page_index` from `wiki.db`.
- The command MUST retrieve each indexed Notion page state via the Notion pages API.
- A page MUST be considered a retirement candidate when `archived=true` or `in_trash=true`.
- The command MUST write timestamped JSON and Markdown reports.
- The report MUST include indexed count, archived candidate count, active count, missing source count, fetch errors, and candidate details.
- The command MUST be dry-run only and MUST NOT mutate DB, Vault, or Mempalace.
- The CLI MUST provide `notion-archived-retirement apply --plan <PATH>`.
- The apply command MUST default to dry-run.
- The apply command MUST require `--apply` before mutating production state.
- Apply MUST process only `apply_safe=true` `retire_notion_source` actions.
- Apply MUST remove the DB source and `notion_page_index` row before deleting matching Vault source files.

## Safety Requirements

- Missing local source rows MUST be reported as `apply_safe=false`.
- Notion API fetch errors MUST be included in the report.
- Token handling MUST rely on `NOTION_TOKEN`; the CLI MUST NOT print tokens.
- Apply MUST validate that `source_id`, `source_uri`, `db_id`, and Notion page ID still match current DB state.
- Apply MUST delete only Vault source Markdown files whose frontmatter source identity matches the applied source.

## Acceptance

- `cargo test -p wiki-storage notion_page_index -- --nocapture`
- `cargo test -p wiki-cli notion_client_retrieves_page_archive_state -- --nocapture`
- `cargo test -p wiki-cli notion_archived_retirement -- --nocapture`
- `cargo test -p wiki-cli --test vault_cli_commands notion_archived_retirement_apply_defaults_to_dry_run_and_apply_retires_source -- --nocapture`
- `cargo test -p wiki-storage snapshot_and_notion_index_deletes_commit_together -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
