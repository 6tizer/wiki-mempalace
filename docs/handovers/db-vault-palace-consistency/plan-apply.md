# Consistency Plan/Apply Handoff

## Scope

- Task: consistency plan schema, validation, dry-run/apply executor.
- Owner files:
  - `crates/wiki-cli/src/consistency.rs`
  - `crates/wiki-cli/tests/consistency.rs`
  - `crates/wiki-cli/src/main.rs`

## Implemented

- `consistency-plan`
  - Requires `consistency-audit-<timestamp>.json`.
  - Generates `consistency-plan-<timestamp>.json/.md`.
  - Markdown is Chinese.
  - Actions are limited to `db_fix`, `vault_cleanup`, `palace_replay`,
    `needs_human`, and `deferred`.
  - Plan validation rejects paths/actions outside audit evidence.

- `consistency-apply`
  - Defaults to dry-run.
  - `--apply` is required for mutation.
  - Validates full plan before first write.
  - Applies DB fixes before Vault projection.
  - For DB link fixes, updates only existing target Vault page files and
    preserves existing frontmatter; it does not create missing legacy/system
    pages during targeted projection.
  - Deletes only audit-listed unmanaged empty files under `pages/` or `sources/`.
  - Replays Mempalace pages through `LiveMempalaceSink::on_page_written`.
  - Does not direct-write `palace.db`.

## Limits

- Resolved legacy Notion export links are executable DB fixes only when audit
  evidence resolves them to a current Vault page path.
- Plain Notion URLs and retired Notion system links are report-only deferred
  records; they are not treated as human review tasks.
- Notion archived state is not yet part of this PR. Known follow-up:
  `sources/wechat/微信公众号文章链接汇总.md` has
  `notion_uuid=7cef8ca26f1645e49158e14520d96bf4`, Notion reports
  `is_archived=true`, but the source still exists in `wiki.db.sources` and
  Vault. Next module should retire archived Notion sources through DB-first
  plan/apply, not by hand-deleting Markdown.
- Source bodies are still not inserted into Mempalace.
- Mempalace replay currently repairs page drawers only.

## Production Run

- Applied plan:
  `/Users/mac-mini/Documents/wiki/reports/consistency-plan-20260426T034229172175Z.json`
- First apply changed DB and replayed Mempalace:
  `db_fixes_applied=305`, `palace_replays_applied=189`.
- Replayed after projection fix:
  `db_fixes_applied=0`, `palace_replays_applied=189`,
  `projection_ran=true`.
- Final audit:
  `/Users/mac-mini/Documents/wiki/reports/consistency-audit-20260426T043735527591Z.json`
  reported `vault_empty_unmanaged=0` and `palace_missing_page_drawers=0`.
- Final plan:
  `/Users/mac-mini/Documents/wiki/reports/consistency-plan-20260426T043743065748Z.json`
  reported `executable_actions=0`; remaining 2012 actions are deferred
  report-only records.

## Verification

```bash
cargo test -p wiki-cli --test consistency
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Result: all passed locally on 2026-04-26.
