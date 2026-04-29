# Design: Audit v2 PR 02 MCP Scope Capability

## Approach

- Replace MCP `resolve_write_scope` with a fallible resolver:
  - missing `scope` returns the server viewer scope;
  - explicit `scope` is parsed with the existing `parse_scope`;
  - parsed scope must equal the server viewer scope;
  - mismatches return typed `scope_denied`.
- Add a MCP helper that checks claim visibility before supersede:
  - `old_claim_id` must exist;
  - old claim scope must be visible to the server viewer using
    `document_visible_to_viewer`;
  - hidden old claims return `scope_denied` before mutation.
- Route all MCP write-capable tools through the same resolver before mutation:
  `wiki_ingest`, `wiki_file_claim`, `wiki_supersede_claim`, `wiki_query`,
  `wiki_promote_claim`, `wiki_crystallize`, `wiki_lint`, `wiki_maintenance`,
  and `wiki_ingest_llm`.
- Run MCP maintenance confidence decay only over viewer-visible claims; hidden
  claims are left unchanged.
- Keep trusted engine APIs unchanged; the hardening is for untrusted MCP client
  input at the boundary.

## Error Shape

- Cross-scope MCP writes return JSON-RPC application error `-32000`.
- `error.data.kind` is `scope_denied`.
- Missing required params and malformed UUIDs keep existing `invalid_params`
  behavior.

## Compatibility

- Existing MCP clients that omit `scope` are unchanged.
- Existing MCP clients that pass a same-scope `scope` are unchanged.
- Existing clients that relied on MCP cross-scope writes must instead start a
  server with the intended `--viewer-scope`.
