# MCP API Reference

统一 MCP server 通过 stdio JSON-RPC 暴露 wiki 与 mempalace 工具。启动示例：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  mcp
```

## Runtime Rules

- Transport: stdio JSON-RPC.
- Input cap: each request line is limited to 10 MiB before JSON parsing.
- Scope default: wiki write tools use server `--viewer-scope` when `scope` is omitted.
- Writer safety: run only one long-lived writer MCP server per `wiki.db`; multiple writers can overwrite each other via stale in-memory snapshots.
- Vault projection: MCP write tools persist DB/outbox state. When the server is started with both `--wiki-dir` and `--sync-wiki`, write tools also refresh `pages/`, `index.md`, and `log.md` through `write_projection`.

## Wiki Tools

| Tool | Required | Optional | Writes | Notes |
| --- | --- | --- | --- | --- |
| `wiki_status` | none | none | no | Returns counts for claims, pages, entities, sources, audits. |
| `wiki_ingest` | `uri`, `body` | `scope`, `tags` | yes | Redacts sensitive content, writes source, persists outbox. |
| `wiki_file_claim` | `text` | `scope`, `tier`, `tags` | yes | Creates claim. Default tier is command implementation default. |
| `wiki_supersede_claim` | `old_claim_id`, `new_text` | `scope`, `tier` | yes | Marks old claim stale and files replacement. |
| `wiki_query` | `query` | `rrf_k`, `per_stream_limit`, `write_page` | yes | Records query state; also writes a page when `write_page=true`. |
| `wiki_promote_claim` | `claim_id` | none | yes | Promotes claim if schema thresholds allow. |
| `wiki_crystallize` | `question` | `findings`, `files`, `lessons`, `entry_type` | yes | Creates a crystallized wiki page and candidate claims. |
| `wiki_lint` | none | none | yes | Runs lint and records lint event. |
| `wiki_wake_up` | none | `max_claims` | no | Returns top semantic claims and recent context. |
| `wiki_maintenance` | none | none | yes | Runs decay, lint, and auto-promote maintenance. |
| `wiki_export_graph_dot` | none | none | no | Returns entity graph DOT text. |
| `wiki_ingest_llm` | `uri`, `body` | `scope`, `dry_run` | yes unless dry-run | Calls configured LLM, validates plan bounds, writes source/claims/summary. Summary page is always `EntryType::Summary`. |

## Mempalace Tools

| Tool | Required | Optional | Writes | Notes |
| --- | --- | --- | --- | --- |
| `mempalace_status` | none | none | no | Returns palace overview. |
| `mempalace_search` | `query` | `wing`, `hall`, `room`, `bank_id`, `limit`, `explain` | no | FTS5 + sparse vector + RRF search. |
| `mempalace_wake_up` | none | `wing`, `bank_id` | no | Returns L0/L1 wake-up context. |
| `mempalace_taxonomy` | none | `bank_id` | no | Returns wing/hall/room tree. |
| `mempalace_traverse` | `wing`, `room` | `bank_id` | no | Follows explicit and implicit tunnels. |
| `mempalace_kg_query` | `subject` | `as_of` | no | Returns active temporal KG facts. |
| `mempalace_kg_timeline` | `subject` | none | no | Returns full timeline for subject. |
| `mempalace_kg_stats` | none | none | no | Returns KG counts. |
| `mempalace_reflect` | `query` | `search_limit`, `bank_id` | no | Searches palace and calls LLM synthesis. |
| `mempalace_extract` | none | `text`, `drawer_id` | no | Extracts SPO triples from text through LLM path; caller decides persistence. |

## Error Shape

Errors use JSON-RPC error responses:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"missing query","data":{"kind":"invalid_params"}}}
```

Current typed error kinds:

| Kind | Code | Meaning |
| --- | --- | --- |
| `parse_error` | `-32700` | Request line is not valid JSON. |
| `method_not_found` | `-32601` | JSON-RPC method is unknown. |
| `invalid_params` | `-32602` | Required tool argument is missing or invalid. |
| `tool_not_found` | `-32602` | `tools/call.name` does not match a known tool. |
| `engine_error` | `-32000` | Wiki engine operation failed. |
| `storage_error` | `-32000` | Repository persistence failed. |
| `llm_error` | `-32000` | LLM config, completion, JSON, validation, or embedding path failed. |
| `mempalace_error` | `-32000` | Mempalace bridge/tool operation failed. |
