# Design: MCP Vault Sync

## Runtime Gate

`main.rs` passes `wiki_root` into `run_mcp` only when `--sync-wiki` is enabled. A bare `--wiki-dir` does not trigger projection.

## Write Path

Replace direct `save_and_flush()` calls in MCP write tools with `save_flush_and_project()`:

1. Persist snapshot and outbox through existing engine policy.
2. If `wiki_dir` is present, call `write_projection(root, &eng.store, &eng.audits)`.
3. Map any projection failure to typed MCP `storage_error`.

## Compatibility

The success JSON returned by MCP tools does not gain projection stats. This keeps MCP clients stable; projection is a side effect governed by startup flags.
