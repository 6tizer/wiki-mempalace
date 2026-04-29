# Requirements: Audit v2 PR 02 MCP Scope Capability

## Goal

Close the P0 MCP scope escalation gap by making MCP write scope server-owned and
checking supersede visibility before mutating claims.

## Functional Requirements

- MCP write tools default writes to the server `--viewer-scope`.
- If a MCP write tool receives an explicit `scope`, it must match the server
  viewer scope exactly.
- Explicit cross-scope writes must fail with a typed MCP error whose
  `error.data.kind` is `scope_denied`.
- `wiki_supersede_claim` must reject superseding an old claim that is not
  visible to the server viewer.
- `wiki_query` write-page, `wiki_crystallize`, `wiki_promote_claim`,
  `wiki_lint`, and `wiki_maintenance` must apply the same explicit-scope
  boundary before mutation.
- `wiki_maintenance` confidence decay must only touch claims visible to the
  server viewer.
- Rejected cross-scope writes must not mutate the in-memory engine, SQLite, or
  Vault projection.

## Non-Goals

- Do not implement a full capability token model; this PR uses exact viewer
  scope as the only allowed MCP write capability.
- Do not change non-MCP CLI write commands.
- Do not change mempalace bank derivation; that is PR 03.
- Do not change production Vault data.

## Acceptance

- A MCP client running under `shared:wiki` cannot write `private:mcp`.
- A MCP client running under `private:a` cannot supersede a `private:b` or
  `shared:wiki` claim.
- A MCP client cannot use `wiki_query.write_page`, `wiki_crystallize`, or
  `wiki_maintenance` to bypass the server viewer scope.
- Same-scope explicit writes still work.
- Default writes still use server viewer scope.
- Workspace fmt, test, and clippy pass.
