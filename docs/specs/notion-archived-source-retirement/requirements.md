# Requirements: Notion Archived Source Retirement

## Functional Requirements

- The CLI MUST provide `notion-archived-retirement plan`.
- The plan command MUST read `notion_page_index` from `wiki.db`.
- The command MUST retrieve each indexed Notion page state via the Notion pages API.
- A page MUST be considered a retirement candidate when `archived=true` or `in_trash=true`.
- The command MUST write timestamped JSON and Markdown reports.
- The report MUST include indexed count, archived candidate count, active count, missing source count, fetch errors, and candidate details.
- The command MUST be dry-run only and MUST NOT mutate DB, Vault, or Mempalace.

## Safety Requirements

- Missing local source rows MUST be reported as `apply_safe=false`.
- Notion API fetch errors MUST be included in the report.
- Token handling MUST rely on `NOTION_TOKEN`; the CLI MUST NOT print tokens.
- Any future apply command MUST retire the DB source first, then update Vault/Palace through existing projection/consumer paths.

## Acceptance

- `cargo test -p wiki-storage notion_page_index -- --nocapture`
- `cargo test -p wiki-cli notion_client_retrieves_page_archive_state -- --nocapture`
- `cargo test -p wiki-cli notion_archived_retirement -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
