# Requirements: MCP Vault Sync

## Functional Requirements

- MCP runtime MUST pass a projection root only when global `--sync-wiki` is enabled.
- MCP write paths that persist DB/outbox state MUST also run `write_projection` when a projection root is present.
- MCP paths that do not persist DB/outbox state MUST NOT write Vault projection.
- Projection failure MUST return a typed MCP `storage_error`.
- Successful MCP result payloads MUST remain unchanged.

## Non-Functional Requirements

- No new runtime dependencies.
- No network calls in tests.
- Projection behavior must reuse existing `write_projection` and vault standards.

## Acceptance

- Tests cover projection after an MCP write with `wiki_dir`.
- Tests cover typed error behavior when projection fails.
- `docs/mcp-api-reference.md` documents the new runtime rule.
