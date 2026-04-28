# Handoff: Notion Archived Source Retirement Audit/Plan

## Branch

`codex/notion-archived-retirement-plan`

## Status

Audit/plan implementation merged in PR #68 on 2026-04-28. Apply implementation merged in PR #69 on 2026-04-28.

## Completed

- Added `WikiRepository::list_notion_page_indexes`.
- Added Notion page archive state retrieval.
- Added `notion-archived-retirement plan`.
- Added timestamped JSON/Markdown dry-run reports.
- Added guarded apply:
  - dry-run by default;
  - `--apply` required for mutation;
  - validates source identity before mutation;
  - removes DB source and `notion_page_index` in one transaction;
  - deletes only matching Vault source Markdown by frontmatter identity.

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

Next roadmap item after PR #69 is `Benchmark Reproducibility`.
