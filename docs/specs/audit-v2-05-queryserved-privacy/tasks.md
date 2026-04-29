# Tasks: Audit v2 PR 05 QueryServed Privacy

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-05-queryserved-privacy/` | Complete |
| QueryServed event schema | Agent | Main | `crates/wiki-core/src/events.rs` | Complete |
| Query producer privacy | Agent | Main | `crates/wiki-kernel/src/engine.rs`, `crates/wiki-cli/src/main.rs` | Complete |
| M12/suggest scoped query-history filter | Agent | Main | `crates/wiki-kernel/src/strategy.rs` | Complete |
| Compatibility test updates | Agent | Main | storage/CLI/bridge tests | Complete |
| Docs / roadmap / handoff | Script | Main | docs | Complete |
| Focused review + gates | Main | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] New events do not serialize raw query text
- [x] New events carry query hash, schema version, and viewer scope
- [x] Old events deserialize
- [x] M12/suggest rejects mismatched event scope
- [x] M12/suggest skips new scoped events when scan has no explicit viewer
- [x] Legacy query-history events remain compatible
- [x] Focused tests added
- [x] Focused review complete
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
