# Requirements: Audit v2 PR 03 Mempalace Bank Capability

## Goal

Close the P0 mempalace bank escalation gap by making wiki MCP mempalace bank
access server-owned and by scoping knowledge-graph reads/writes to the same
bank boundary.

## Functional Requirements

- `wiki-cli` MCP `mempalace_*` tools must not expose `bank_id` as a client
  input for bank-scoped tools.
- `wiki-cli` MCP must derive mempalace `bank_id` from the server
  `--viewer-scope`:
  - `shared:<team>` maps to `<team>`.
  - `private:<agent>` maps to `<agent>`.
- If a `wiki-cli` MCP client still sends `bank_id`, the request must fail with
  typed `scope_denied`.
- Mempalace search, wake-up, taxonomy, traverse, reflect, status, extract, KG
  query, KG timeline, and KG stats must use the derived bank.
- `kg_facts` must carry `bank_id`; existing databases must migrate
  idempotently with `default`.
- Explicit `tunnels` must carry `bank_id`; existing tunnels must migrate
  idempotently with `default`.
- Live wiki-to-mempalace KG writes must assign the sink bank and invalidate only
  facts in that bank.
- SearchPorts / graph rank extras must not read KG facts from another bank.

## Non-Goals

- Do not change the standalone `rust-mempalace` local MCP server into a
  server-owned capability model; it has no wiki viewer scope.
- Do not change drawer dedupe from global hash to `(bank_id, content_hash)`;
  that is PR 04.
- Do not run production palace migrations against `/Users/mac-mini/Documents/wiki`.

## Acceptance

- A `wiki-cli` MCP client cannot select another mempalace bank by sending
  `bank_id`.
- `shared:wiki` and `private:<agent>` map deterministically to mempalace bank
  IDs.
- KG query/timeline/stats return only facts for the selected bank.
- Bridge live sink KG facts are written under the sink bank.
- Focused tests and workspace fmt/test/clippy pass.
