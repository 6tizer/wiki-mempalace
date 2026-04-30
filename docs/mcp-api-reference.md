# MCP API Reference

统一 MCP server 通过 stdio JSON-RPC 暴露 wiki 与 mempalace 工具。本文描述
`cargo run -p wiki-cli -- ... mcp` 入口的当前合同。

启动示例：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  mcp
```

## Runtime Rules

- Transport: stdio JSON-RPC 2.0 line protocol.
- Request cap: each request line is limited to 10 MiB before JSON parsing.
- Tool discovery: `tools/list` returns all `wiki_*` and `mempalace_*` tools in one array.
- Limit clamp: `wiki_query.per_stream_limit`, `mempalace_search.limit`, and
  `mempalace_reflect.search_limit` are clamped to `1..=100`.
- Wiki write scope: wiki write tools use server `--viewer-scope` when `scope` is
  omitted. If client sends `scope`, it must exactly match server viewer scope or
  the request fails with `scope_denied`.
- Mempalace bank: mempalace bank is derived only from server `--viewer-scope`
  (`private:<agent> -> <agent>`, `shared:<team> -> <team>`). client `bank_id` is rejected with `scope_denied`; it is not a capability argument.
- Query backend: `wiki_query` uses storage-backed `SqliteSearchPorts` by default
  and composes `MempalaceSearchPorts` when `--palace` opens. If palace open
  fails, wiki storage results still return. If storage open fails, it falls back
  to the in-memory query path and logs a warning to stderr.
- Vault projection: MCP write tools persist DB/outbox state. When started with
  both `--wiki-dir` and `--sync-wiki`, write tools refresh `pages/`, `index.md`,
  and `log.md` through `write_projection`.
- Governance/Fixer/Synthesis automation is CLI-only in this version. MCP does
  not expose `fixer-apply` or `research-synthesis run` because those are batch
  jobs with reports, writer lease semantics, and optional web/LLM calls.

## Wiki Tools

| Tool | Required | Optional | Writes | Returns | Notes |
| --- | --- | --- | --- | --- | --- |
| `wiki_status` | none | none | no | `claims`, `pages`, `entities`, `sources`, `audit_records` | In-memory snapshot counts. |
| `wiki_ingest` | `uri`, `body` | `scope`, `tags` | yes | `source_id` | Redacts sensitive content; writes source, outbox, optional embeddings, optional projection. |
| `wiki_file_claim` | `text` | `scope`, `tier`, `tags` | yes | `claim_id` | `tier` is `working`, `episodic`, `semantic`, or `procedural`; default `working`. |
| `wiki_supersede_claim` | `old_claim_id`, `new_text` | `scope`, `tier` | yes | `new_claim_id` | Old claim must be visible to server viewer scope. |
| `wiki_query` | `query` | `rrf_k`, `per_stream_limit`, `write_page`, `scope` | yes | `results[{doc_id, score}]` | Always records `QueryServed`; writes page only when `write_page=true`. |
| `wiki_promote_claim` | `claim_id` | `scope` | yes | `claim_id`, `tier` | Promotion follows schema thresholds and viewer visibility. |
| `wiki_crystallize` | `question` | `findings`, `files`, `lessons`, `entry_type`, `scope` | yes | `page_id`, `page_title`, `claim_candidates` | Creates a wiki page and candidate claims. |
| `wiki_lint` | none | `scope` | yes | `findings[{severity, code, message, subject}]` | Runs basic lint, records lint event, optional projection. |
| `wiki_wake_up` | none | `max_claims` | no | `context` | Returns semantic/procedural claims and recent visible pages. |
| `wiki_maintenance` | none | `scope` | yes | `decay_applied`, `claims_decayed`, `lint_findings`, `claims_promoted` | Decay + lint + auto-promote for visible scope. |
| `wiki_export_graph_dot` | none | none | no | `dot` | Exports visible entity graph as DOT text. |
| `wiki_ingest_llm` | `uri`, `body` | `scope`, `dry_run` | yes unless dry-run | dry-run: `plan`; apply: `source_id`, `claims_filed`, `summary`, `summary_page_id` | Uses configured LLM; validates plan bounds; summary page is fixed to `EntryType::Summary`. |

## Mempalace Tools

All mempalace tools run under the derived server bank. They do not accept a
client bank capability argument.

| Tool | Required | Optional | Writes | Returns | Notes |
| --- | --- | --- | --- | --- | --- |
| `mempalace_status` | none | none | no | `drawers`, `wings`, `tunnels`, `kg_facts` | Overview filtered by derived bank when live palace is configured. |
| `mempalace_search` | `query` | `wing`, `hall`, `room`, `limit`, `explain` | no | `results` | FTS5 + sparse vector + RRF search; limit clamps to `1..=100`. |
| `mempalace_wake_up` | none | `wing` | no | `context` | L0 identity + L1 critical facts. |
| `mempalace_taxonomy` | none | none | no | `taxonomy` | Wing/hall/room tree and counts. |
| `mempalace_traverse` | `wing`, `room` | none | no | `nodes`, `edges` | Follows explicit tunnels and implicit same-room links. |
| `mempalace_kg_query` | `subject` | `as_of` | no | `facts` | Returns active temporal KG facts. |
| `mempalace_kg_timeline` | `subject` | none | no | `timeline` | Returns subject timeline across fact validity windows. |
| `mempalace_kg_stats` | none | none | no | `subjects`, `predicates`, `active_facts` | KG counts for derived bank. |
| `mempalace_reflect` | `query` | `search_limit` | no | `text` | RAG synthesis through configured mempalace LLM path; limit clamps to `1..=100`. |
| `mempalace_extract` | none | `text`, `drawer_id` | yes in live mode | `kg_facts_added` | Provide exactly one of `text` or `drawer_id`; live mode extracts SPO triples into KG. |

## Side Effects

| Category | Tools |
| --- | --- |
| Wiki DB/outbox write | `wiki_ingest`, `wiki_file_claim`, `wiki_supersede_claim`, `wiki_query`, `wiki_promote_claim`, `wiki_crystallize`, `wiki_lint`, `wiki_maintenance`, `wiki_ingest_llm` apply mode |
| Vault projection possible | Same wiki write tools when `--wiki-dir --sync-wiki` are enabled |
| Query event only | `wiki_query` when `write_page=false` |
| LLM call | `wiki_ingest_llm`, `mempalace_reflect`, `mempalace_extract` |
| Palace write | `mempalace_extract` in live mode |
| Read-only | `wiki_status`, `wiki_wake_up`, `wiki_export_graph_dot`, all mempalace tools except `mempalace_extract` |

## CLI-Only Governance Surface

The following capabilities are available through `wiki-cli`, not MCP:

| Command | Writes | Notes |
| --- | --- | --- |
| `governance scan` | no | Produces lifecycle/reference/lint/duplicate/tag/synthesis signals. |
| `governance fixer-plan` | no | Builds typed evidence fix plans from scan JSON. |
| `governance fixer-apply --policy evidence-auto --apply` | yes | Applies only ready actions; merge/retire/patch actions produce tombstones. |
| `governance restore --tombstone ... --apply` | yes | Restores from fixer tombstone through DB -> Vault projection -> outbox. |
| `research-synthesis discover` | no | Finds single/double/triple/quad tag synthesis candidates. |
| `research-synthesis compose/run --apply` | yes | Writes verified `entry_type=synthesis` pages. |
| `automation run-daily` | yes | Runs the daily maintenance lane. |

## Error Shape

Errors use JSON-RPC error responses:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "missing query",
    "data": { "kind": "invalid_params" }
  }
}
```

Current typed error kinds:

| Kind | Code | Meaning |
| --- | --- | --- |
| `parse_error` | `-32700` | Request line is not valid JSON. |
| `method_not_found` | `-32601` | JSON-RPC method is unknown. |
| `invalid_params` | `-32602` | Required argument is missing or invalid. |
| `tool_not_found` | `-32602` | `tools/call.name` does not match a known tool. |
| `scope_denied` | `-32000` | Client requested a scope or mempalace bank outside server capability. |
| `engine_error` | `-32000` | Wiki engine operation failed. |
| `storage_error` | `-32000` | Repository persistence or projection failed. |
| `llm_error` | `-32000` | LLM config, completion, JSON, validation, or embedding path failed. |
| `mempalace_error` | `-32000` | Mempalace bridge/tool operation failed. |

## Compatibility Notes

- Result shapes are intentionally high-level in this reference; callers should
  treat additional fields as additive.
- `wiki_ingest_llm.entry_type` is no longer a public schema argument; summary
  pages are fixed to `EntryType::Summary`.
- Standalone `rust-mempalace mcp` may expose local bank filters. This reference
  covers the unified `wiki-cli mcp` server, where bank is server-derived.
