# Tasks: Audit v2 PR 08 Production SearchPorts Default

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-08-production-searchports/` | Complete |
| Storage-backed SearchPorts | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| CLI query/explain wiring | Agent | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Query truth table docs | Script | Main | `docs/mempalace-linkage.md` | Complete |
| Focused tests | Agent | Main | `wiki-storage`, `wiki-cli` tests | Complete |
| Retrieval quality review + gates | Main/Subagent | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] `query` default wiki stream uses `SqliteSearchPorts`
- [x] `explain` wiki stream uses `SqliteSearchPorts`
- [x] Valid palace path composes storage-backed wiki + mempalace ports
- [x] Invalid palace path falls back to wiki-only storage-backed ports
- [x] `InMemorySearchPorts` remains fallback/test support
- [x] Query truth table updated
- [x] Retrieval quality review complete; P1 graph extras finding fixed
- [x] `cargo test -p wiki-storage sqlite_search_ports_read_snapshot_rows_and_filter_scope -- --nocapture`
- [x] `cargo test -p wiki-cli storage_ports -- --nocapture`
- [x] `cargo test -p wiki-cli graph_extras -- --nocapture`
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo deny --all-features check advisories bans licenses sources`
