# Tasks: Notion Full Reimport 2026-05-05

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Scanner filtering/reporting | Main | `crates/wiki-migration-notion/src/scanner.rs`, `report.rs`, `main.rs` | Complete |
| Case-insensitive writer collision handling | Main | `crates/wiki-migration-notion/src/writer.rs` | Complete |
| Backfill metadata preservation | Main | `crates/wiki-cli/src/vault_backfill.rs`, `crates/wiki-cli/tests/vault_backfill.rs` | Complete |
| Staging rebuild | Main | `/Users/mac-mini/Documents/wiki-migration-staging/notion-export-20260505` | Complete |
| Production backup and replacement | Main | `/Users/mac-mini/Documents/wiki` | Complete |
| Docs/handoff/roadmap | Main | `docs/` | Complete |

## Validation

- `cargo test -p wiki-migration-notion`
- `cargo test -p wiki-cli --test vault_backfill`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace` (partially run before user interruption; completed slices all passed; `suggest` was re-run directly)
- `cargo clippy --workspace --all-targets -- -D warnings`

Cargo commands must be run serially.

## Staging Acceptance

- Dry-run counts: Wiki `4765`, X `943`, WeChat `583`.
- Migrate stats: pages `4765`, sources `1526`, collisions resolved `3`.
- Backfill report: `sources_seen=1526`, `pages_seen=4765`, `skipped=0`.
- DB snapshot: `sources=1526`, `pages=4765`.
- `notion_page_index`: X + WeChat total `1526`.
- Query, palace, lint, and consistency smoke checks pass with no P0/P1 issues.

## Production Result

- Backup: `/Users/mac-mini/Documents/wiki-backups/notion-full-reimport-20260505-20260505-071121/wiki`.
- Old swapped directory:
  `/Users/mac-mini/Documents/wiki-old-notion-full-reimport-20260505-20260505-071121`.
- Production DB: `sources=1526`, `pages=4765`.
- Production Vault: `pages/**/*.md=4765`, `sources/**/*.md=1526`.
- Production `notion_page_index`: `x_bookmark=943`, `wechat=583`.
- Production query smoke returned both `page:*` and `mp_drawer:*` hits.
- Production consistency audit:
  `db_pages=4765 db_sources=1526 vault_empty_unmanaged=0 palace_missing_page_drawers=0`.
