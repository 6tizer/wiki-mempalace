# Consistency Audit Handoff

## Scope

- Task: DB/Vault/Mempalace consistency audit.
- Owner files:
  - `crates/wiki-cli/src/consistency.rs`
  - `crates/wiki-cli/tests/consistency.rs`
  - `crates/wiki-cli/src/main.rs`

## Implemented

- `consistency-audit` CLI command.
- Reads `wiki.db` through `WikiRepository::load_snapshot`.
- Scans only Vault `pages/` and `sources/`.
- Optional Mempalace read compares DB pages to `wiki://page/<page_id>` drawers.
- Reports source drawer policy as out of scope, not as a missing condition.
- Writes timestamped reports:
  - `consistency-audit-<timestamp>.json`
  - `consistency-audit-<timestamp>.md`

## Covered Findings

- DB page/source counts.
- Vault managed/missing/extra page/source identities.
- Empty unmanaged files under `pages/` / `sources/`.
- Stale Notion-style links in DB page markdown.
- Unresolved wiki links in DB page markdown.
- Exact source-summary candidates.
- Missing/stale Mempalace page drawers.

## Verification

```bash
cargo test -p wiki-cli --test consistency
```

Result: 10 passed.
