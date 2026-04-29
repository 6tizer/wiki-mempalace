# Tasks: Audit v2 PR 03 Mempalace Bank Capability

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-03-mempalace-bank-capability/` | Complete |
| MCP bank resolver | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| MCP schema cleanup | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Bridge trait bank params | Agent | Main | `crates/wiki-mempalace-bridge/src/tools.rs`, `live_tools.rs` | Complete |
| KG/tunnel schema migration | Agent | Main | `crates/rust-mempalace/src/db.rs` | Complete |
| KG service bank filters | Agent | Main | `crates/rust-mempalace/src/service.rs` | Complete |
| KG extract bank routing | Agent | Main | `crates/rust-mempalace/src/service.rs`, `crates/wiki-mempalace-bridge/src/live_tools.rs` | Complete |
| Live sink / graph ports bank routing | Agent | Main | `crates/wiki-mempalace-bridge/src/live_sink.rs`, `live_ranker.rs`, `live_search.rs` | Complete |
| Tests | Agent | Main | MCP + KG bank tests | Complete |
| Roadmap / handoff | Script | Main | `docs/roadmap.md`, handoff | Complete |
| Focused review + gates | Skill | Main | workspace | In Progress |

## Checklist

- [x] Behavior matches roadmap scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope drawer dedupe left for PR 04
- [x] wiki MCP client `bank_id` rejected
- [x] wiki MCP mempalace bank derived from viewer scope
- [x] KG facts carry bank ID
- [x] Explicit tunnels carry bank ID
- [x] KG query/timeline/stats are bank-scoped
- [x] Bridge live sink writes KG facts under sink bank
- [x] Focused tests added
- [x] Security-focused review completed; P1/P2 traverse/status + migration findings fixed
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
