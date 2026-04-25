# Requirements: Agent Runtime Defaults

## Goal

- Make normal CLI/MCP agent usage target the shared vault-local knowledge
  system.

## Plain-Language Summary

- What this module does: fixes the daily path so agents do not accidentally use
  repo-root empty `wiki.db` or private scopes.
- Who it talks to: CLI flags, MCP startup docs, AGENTS workflow, and runbooks.
- What user decision it implements: multiple agents share `shared:wiki`.

## Functional Requirements

- Document canonical paths:
  - vault: `/Users/mac-mini/Documents/wiki`
  - wiki db: `/Users/mac-mini/Documents/wiki/.wiki/wiki.db`
  - palace db: `/Users/mac-mini/Documents/wiki/.wiki/palace.db`
  - scope: `shared:wiki`
- Provide exact commands for:
  - audit
  - backfill dry-run
  - limited backfill
  - full backfill
  - palace init
  - query
  - explain
  - metrics/dashboard
  - MCP server startup
- Make MCP write/read behavior explicit.
- If code defaults are changed, preserve explicit flag overrides.
- If code defaults are not changed, docs must make required flags impossible to
  miss.

## Non-Goals

- Do not remove private scope support.
- Do not break existing tests that rely on default temp DBs.
- Do not introduce a web server.

## Inputs / Outputs

- Input: final B1-B3 command shape.
- Output: docs and optional CLI/MCP default changes.

## Acceptance Criteria

- A new agent can start MCP against the shared vault-local dbs from docs alone.
- Query/write examples use `shared:wiki`.
- No command points at repo-root `wiki.db` for the production vault path.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: changing global defaults outside this repo.
- Agent can automate: repo docs, CLI tests, MCP default tests if in scope.
