# Handover: Audit Disposition PR1 MCP Query Storage Ports

## Scope

M-5 from the 2026-04-30 audit disposition: MCP `wiki_query` now prefers
SQLite-backed search ports.

## Changed Files

- `crates/wiki-cli/src/mcp.rs`
- `docs/prd/audit-disposition-2026-04-30.md`
- `docs/specs/audit-disposition-01-mcp-query-storage-ports/*`
- `docs/roadmap.md`
- `docs/prd/README.md`
- `docs/specs/README.md`
- `docs/LESSONS.md`

## Behavior

- MCP `wiki_query` reloads the engine from `wiki.db`, opens `SqliteSearchPorts`,
  and runs `query_pipeline_with_ports`.
- If `--palace` opens, the query uses `CompositeSearchPorts(Sqlite, Mempalace)`.
- If palace cannot open, the query falls back to storage-backed wiki-only.
- If storage reload/open fails, the query falls back to the old memory path.
- `QueryServed` event recording and `write_page=true` remain compatible.

## Verification

Passed local gates on 2026-04-30:

- `cargo test -p wiki-cli mcp_query -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo deny --all-features check advisories bans licenses sources`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Focused Review

- Scope: on target.
- Review depth: standard.
- Hard stops: 0.
- No production `/Users/mac-mini/Documents/wiki` write operation was run.

## Production Safety

No production `/Users/mac-mini/Documents/wiki` write operation is part of this PR.
