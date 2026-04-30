# Design: Audit Disposition PR3 MCP API Reference

## Summary

The MCP API reference had drifted from the current unified `wiki-cli mcp`
server. It listed old mempalace client bank args and did not cover every
runtime rule. This PR rewrites the reference and adds tests that compare docs
against `tools_list()`.

## Plain-Language Design

- Module role: keep the public MCP reference aligned with runtime schema.
- Data it asks for: `tools_list()`, handler behavior, bridge tool behavior.
- Data it returns: refreshed Markdown docs plus sync tests.

## API Contract

- Wiki tools keep their existing schema and result shapes.
- Mempalace tools remain server-bank scoped; client bank override is rejected.
- Result shape docs stay high-level to allow additive fields.
- Typed error kinds are listed from current `McpToolError` mapping.

## Test Design

- `mcp_api_reference_mentions_all_tools_and_error_kinds`: every `tools_list()`
  name and every current typed error kind must appear in the reference.
- `mcp_api_reference_does_not_list_mempalace_bank_id_as_client_arg`: mempalace
  tool Required/Optional columns must not expose client `bank_id`, while runtime
  rules still document rejection.

## Compatibility

- Docs and tests only.
- No runtime behavior change.
- Standalone `rust-mempalace mcp` remains outside this reference.

## Review Focus

- Check that write/read-only side effects are accurate.
- Check that `mempalace_extract` is marked as a live-mode palace write.
- Check that bank capability wording does not re-open client bank injection.
