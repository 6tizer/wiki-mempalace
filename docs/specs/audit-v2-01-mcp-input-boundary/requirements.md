# Requirements: Audit v2 PR 01 MCP Input Boundary

## Goal

Close the first small P0 audit gap by bounding MCP result limits and preventing
lint report path traversal.

## Functional Requirements

- Clamp MCP result limits to `1..=100` for:
  - `wiki_query.per_stream_limit`
  - `mempalace_search.limit`
  - `mempalace_reflect.search_limit`
- Expose the same bounds in MCP tool schemas.
- Reject non-integer limit values as invalid params.
- Reject lint report names that contain path separators, absolute paths, hidden
  names, or `..`.
- Write accepted lint reports only below `<wiki-root>/reports/`; reject reports
  directory symlinks and avoid following existing report-file symlinks.
- Apply the same limit policy to the standalone `rust-mempalace` MCP server.

## Non-Goals

- Do not change MCP scope semantics; that is PR 02.
- Do not change mempalace bank derivation; that is PR 03.
- Do not change production Vault data.

## Acceptance

- `../evil`, absolute paths, nested lint report names, and reports directory
  symlinks fail with `InvalidInput`.
- Existing report-file symlinks are replaced without overwriting their targets.
- Oversized MCP limits are clamped to `100`; `0` is clamped to `1`.
- Non-integer MCP limits return invalid params / tool error.
- Existing default limits remain unchanged.
- Workspace fmt, test, and clippy pass.
