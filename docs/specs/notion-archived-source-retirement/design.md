# Design: Notion Archived Source Retirement

## Command

Plan:

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  notion-archived-retirement plan
```

Apply dry-run:

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  notion-archived-retirement apply \
  --plan /Users/mac-mini/Documents/wiki/reports/notion-archived-retirement-plan-<timestamp>.json
```

Apply:

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  notion-archived-retirement apply \
  --plan /Users/mac-mini/Documents/wiki/reports/notion-archived-retirement-plan-<timestamp>.json \
  --apply
```

Options:

- `--report-dir <PATH>`: defaults to `<wiki-dir>/reports` when `--wiki-dir` is set, otherwise `reports`.
- `--limit <N>`: inspect only the first N indexed pages.
- `--request-delay-ms <MS>`: Notion request delay, minimum enforced by the client.

## Data Flow

1. `wiki-storage` lists `notion_page_index` rows in stable order.
2. `wiki-cli` calls `GET /v1/pages/{page_id}` and reads `id`, `archived`, and `in_trash`.
3. The planner joins archive state to `eng.store.sources` by `source_id`.
4. The report module writes:

```text
<report-dir>/
  notion-archived-retirement-plan-<timestamp>.json
  notion-archived-retirement-plan-<timestamp>.md
```

## Report Contract

JSON is the machine source of truth. Markdown is the sibling human view.

Candidate fields:

- `action_type=retire_notion_source`
- `db_id`
- `notion_page_id`
- `notion_api_page_id`
- `source_id`
- `source_uri`
- `title`
- `archived`
- `in_trash`
- `synced_at`
- `reason`
- `apply_safe`

## Apply Boundary

Apply reads the JSON plan and filters to `apply_safe=true`.

Validation before mutation:

- `action_type` must be `retire_notion_source`.
- `source_id` must still exist in `wiki_state.sources`.
- `source_uri`, `db_id`, and canonical Notion page ID must still match the plan.

Mutation order:

1. Remove the source from `wiki_state.sources`.
2. Delete the matching `notion_page_index` row in the same SQLite transaction.
3. Delete matching Vault `sources/**.md` files only when their frontmatter `source_id` or `notion_uuid` matches the applied source.
4. Write timestamped apply JSON/Markdown reports.

Compiled wiki pages are not deleted by source retirement. If compiled knowledge needs pruning, that must be a separate plan with page-level evidence.
