# Audit v2 PR 03 Mempalace Bank Capability Handoff

## Summary

Branch: `codex/audit-v2-03-mempalace-bank-capability`

This PR implements Audit Report Follow-up v2 item 3:

- `wiki-cli` MCP mempalace tools derive bank from server viewer scope.
- Client-provided `bank_id` is rejected with typed `scope_denied`.
- `kg_facts` now has a `bank_id` column with idempotent migration.
- `tunnels` now has a `bank_id` column with idempotent migration.
- KG query, timeline, stats, and invalidate support bank filters.
- Explicit traverse edges and status tunnel counts are bank-scoped.
- KG extract writes facts to the selected bank and filters drawer extraction by
  bank when a bank is provided.
- Bridge live sink writes KG facts under the sink bank.
- SearchPorts and graph-rank extras use the configured bank for KG reads.

## Files Changed

- `crates/wiki-cli/src/mcp.rs`
- `crates/rust-mempalace/src/db.rs`
- `crates/rust-mempalace/src/service.rs`
- `crates/rust-mempalace/src/main.rs`
- `crates/rust-mempalace/src/mcp.rs`
- `crates/wiki-mempalace-bridge/src/tools.rs`
- `crates/wiki-mempalace-bridge/src/live_tools.rs`
- `crates/wiki-mempalace-bridge/src/live_sink.rs`
- `crates/wiki-mempalace-bridge/src/live_ranker.rs`
- `crates/wiki-mempalace-bridge/src/live_search.rs`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/specs/audit-v2-03-mempalace-bank-capability/`
- `docs/handovers/audit-v2-03-mempalace-bank-capability/summary.md`
- `docs/LESSONS.md`

## Interfaces

- `wiki-cli` MCP mempalace tools no longer treat client `bank_id` as trusted.
- Standalone `rust-mempalace` CLI remains global for existing KG commands.
- Standalone `rust-mempalace` MCP keeps optional local `bank_id` filters,
  including KG tools.
- KG service APIs accept optional bank filters for trusted callers.

## Known Limits

- Drawer dedupe is still globally keyed by `content_hash`; PR 04 changes it to
  `(bank_id, content_hash)`.
- Existing KG rows migrate to `default`; no production migration is executed in
  this PR.
- Full capability tokens / delegated banks are not implemented.

## Verification

Focused checks:

- `cargo test -p rust-mempalace kg_queries_and_stats_are_bank_scoped`
- `cargo test -p wiki-cli mempalace_`
- `cargo test -p wiki-mempalace-bridge`

Security review initially found explicit tunnel leakage through traverse/status,
then a legacy migration ordering bug for bank indexes. Both classes are fixed
with bank-owned tunnels, migration ordering, and regression coverage.

Required full gates passed locally:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
