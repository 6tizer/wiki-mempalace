# Audit v2 PR 02 MCP Scope Capability Handoff

## Summary

Branch: `codex/audit-v2-02-mcp-scope-capability`

This PR implements Audit Report Follow-up v2 item 2:

- MCP write tools no longer allow arbitrary client-side scope override.
- Missing `scope` still defaults to server `--viewer-scope`.
- Explicit `scope` must exactly match the server viewer scope.
- Cross-scope write attempts return typed MCP `scope_denied`.
- `wiki_supersede_claim` rejects old claims that are hidden from the server
  viewer before mutating state.
- `wiki_query` write-page, `wiki_crystallize`, `wiki_promote_claim`,
  `wiki_lint`, and `wiki_maintenance` also enforce explicit scope before
  mutation.
- `wiki_maintenance` confidence decay only touches viewer-visible claims.

## Files Changed

- `crates/wiki-cli/src/mcp.rs`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/specs/audit-v2-02-mcp-scope-capability/`
- `docs/handovers/audit-v2-02-mcp-scope-capability/summary.md`
- `docs/LESSONS.md`

## Interfaces

- MCP write `scope` is now a capability boundary, not a free override.
- `error.data.kind = "scope_denied"` is returned for scope mismatch and hidden
  old-claim supersede attempts.
- Non-MCP CLI write commands are unchanged.

## Known Limits

- Full capability token / delegated sub-scope model is not implemented.
- Mempalace bank derivation remains PR 03.
- Cross-bank dedupe remains PR 04.

## Verification

Focused checks already run:

- `cargo test -p wiki-cli mcp_`
- `cargo fmt --all -- --check`
- `cargo clippy -p wiki-cli --all-targets -- -D warnings`

Security-focused review found two missed write entrypoints: maintenance
cross-viewer decay and write-page/crystallize explicit scope mismatch. Both are
fixed with regression tests.

Full workspace gate has passed:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`

Security-focused re-review found no remaining PR2 P0/P1/P2 findings.
