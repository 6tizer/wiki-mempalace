# Handoff: Notion Archived Source Retirement Audit/Plan

## Branch

`codex/notion-archived-retirement-plan`

## Status

Audit/plan implementation merged in PR #68 on 2026-04-28.

## Completed

- Added `WikiRepository::list_notion_page_indexes`.
- Added Notion page archive state retrieval.
- Added `notion-archived-retirement plan`.
- Added timestamped JSON/Markdown dry-run reports.
- Kept apply out of scope.

## Command

```bash
NOTION_TOKEN="$(security find-generic-password -a NOTION_TOKEN -s wiki-mempalace -w)" \
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  notion-archived-retirement plan
```

## Output

Reports write to `<wiki-dir>/reports/` by default.

## Next PR

Implement guarded apply from the generated JSON plan:

- only `apply_safe=true`;
- DB-first mutation;
- Vault projection after DB change;
- Mempalace consumer/validation after projection;
- manual-review rows remain untouched.
