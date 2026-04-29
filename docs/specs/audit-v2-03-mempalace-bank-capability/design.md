# Design: Audit v2 PR 03 Mempalace Bank Capability

## Approach

- Add a `wiki-cli` MCP bank resolver beside the existing scope resolver:
  - derive bank from `Scope::Shared.team_id` or `Scope::Private.agent_id`;
  - reject any client-provided `bank_id` before dispatching mempalace tools.
- Remove `bank_id` from the `wiki-cli` MCP input schema for bank-scoped
  mempalace tools. The standalone `rust-mempalace` MCP schema keeps optional
  `bank_id` because it is a local tool without wiki viewer context.
- Extend `MempalaceTools` so status and KG methods accept an optional bank,
  matching existing search/taxonomy/traverse/reflect bank filtering.
- Add `kg_facts.bank_id` and `tunnels.bank_id` with idempotent SQLite
  migrations.
- Change KG service functions:
  - `kg_add` resolves bank from explicit bank, source drawer bank, or
    `default`;
  - `extract_to_kg` writes extracted facts to the provided bank, and drawer
    extraction can filter `drawer_id` by bank;
  - `kg_query`, `kg_timeline`, and `kg_stats` filter by bank when provided;
  - `kg_invalidate` can invalidate only within a provided bank.
- Filter explicit traverse edges and status tunnel counts by `tunnels.bank_id`.
- Route bridge live sink and graph/search ports through configured bank IDs.

## Compatibility

- Existing KG rows migrate to `bank_id = 'default'`.
- Existing local `rust-mempalace` CLI behavior remains global unless a caller
  explicitly uses bank-aware APIs through MCP.
- `wiki-cli` MCP clients that previously sent `bank_id` must instead start the
  MCP server with the intended `--viewer-scope`.

## Security Boundary

`wiki-cli` MCP is the untrusted boundary. Client input cannot choose palace
bank; only the server viewer scope can. Downstream service APIs still accept an
optional bank so trusted local callers can choose global or scoped behavior.
