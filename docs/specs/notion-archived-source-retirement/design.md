# Design: Notion Archived Source Retirement

## Command

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  notion-archived-retirement plan
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

This PR does not implement apply. The next PR should read the JSON plan, filter `apply_safe=true`, mutate `wiki.db` first, then run Vault projection and Mempalace consumer validation.
