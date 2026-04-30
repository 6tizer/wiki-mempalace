# Requirements: Audit Disposition PR1 MCP Query Storage Ports

## Goal

Make MCP `wiki_query` use persisted SQLite-backed wiki search ports by default,
matching CLI `query` / `explain` production behavior.

## Plain-Language Summary

- What this module does: MCP query should search the durable `wiki.db` snapshot, not only the current in-memory engine store.
- Who it talks to: MCP handler, SQLite repository, optional Mempalace search ports.
- What user decision it implements: M-5 “待修复”.

## Functional Requirements

- MCP `wiki_query` must prefer `SqliteSearchPorts::open(repo, Some(viewer))`.
- If `--palace` is configured and can open, MCP query must combine wiki storage ports with `MempalaceSearchPorts`.
- If storage ports cannot open, MCP query may fall back to existing `query_pipeline_memory`.
- If palace cannot open, MCP query must continue with storage-backed wiki-only results.
- `QueryServed` hash/scope event recording must remain.
- `write_page=true` must still create a page and projection when `--sync-wiki` is active.
- Existing JSON result shape must remain unchanged.

## Non-Goals

- Do not change CLI `query` / `explain`.
- Do not change result schema or add public MCP arguments.
- Do not run production wiki writes.

## Inputs / Outputs

- Input: MCP `wiki_query` args: `query`, `rrf_k`, `per_stream_limit`, `write_page`, `scope`.
- Output: existing `results` array with `doc_id` and `score`.

## Acceptance Criteria

- A persisted claim/page can be returned even when the current in-memory store starts stale or empty.
- Query persistence does not erase the persisted snapshot.
- Invalid palace path does not fail the MCP query.
- `cargo test -p wiki-cli mcp_query -- --nocapture` passes.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: none; roadmap disposition already approved.
- Agent can automate: implementation, review, tests, PR, CI, merge.
