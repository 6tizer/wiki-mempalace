# Agent Runtime Defaults Handoff

## Scope

- Owner: Subagent D.
- Files changed: `crates/wiki-cli/src/mcp.rs`, `AGENTS.md`, this handoff.
- Branch: `codex/vault-backfill-palace-init`.

## Changes

- MCP write tools now default missing `scope` to the MCP server `--viewer-scope`.
- Explicit `scope` arguments still override the server viewer scope.
- `AGENTS.md` now shows the shared vault-local runtime tuple:
  - `--db /Users/mac-mini/Documents/wiki/.wiki/wiki.db`
  - `--wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki`
  - `--viewer-scope shared:wiki`
  - `--palace /Users/mac-mini/Documents/wiki/.wiki/palace.db`
- Architecture/outbox/mempalace docs now avoid production commands pointing at
  repo-root `wiki.db`.

## Tests

- Added unit coverage for default MCP writes under `shared:wiki`.
- Added explicit override coverage for `scope: private:mcp`.

## Notes

- Did not touch `main.rs`; CLI command wiring remains main-agent scope.
- Existing private scope support remains intact.

## Review Follow-up

- RD P2 fixed: spec now says omitted MCP write scope uses server
  `--viewer-scope`, not `private:mcp`.
- RD P3 fixed: task status回填 completed items.
