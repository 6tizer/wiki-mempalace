# Tasks: Audit v2 PR 02 MCP Scope Capability

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-02-mcp-scope-capability/` | Complete |
| Typed scope denied error | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| MCP write scope resolver | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Supersede visibility guard | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Secondary MCP write entrypoints | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Tests | Agent | Main | MCP scope tests | Complete |
| Roadmap / handoff | Script | Main | `docs/roadmap.md`, `docs/handovers/audit-v2-02-mcp-scope-capability/summary.md` | Complete |
| Focused review + gates | Skill | Main | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior left for later PRs
- [x] Cross-scope MCP writes rejected
- [x] Hidden old-claim supersede rejected
- [x] Query write-page / crystallize mismatch rejected before mutation
- [x] Maintenance decay limited to viewer-visible claims
- [x] Same-scope explicit writes preserved
- [x] `cargo fmt --all -- --check`
- [x] Security-focused review completed; P1/P2 write-entrypoint findings fixed
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
