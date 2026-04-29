# Tasks: Audit v2 PR 04 Cross-Bank Drawer Dedupe

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-04-cross-bank-dedupe/` | Complete |
| Drawer unique index migration | Agent | Main | `crates/rust-mempalace/src/db.rs` | Complete |
| Mine path duplicate checks | Agent | Main | `crates/rust-mempalace/src/service.rs` | Complete |
| Live sink duplicate check | Agent | Main | `crates/wiki-mempalace-bridge/src/live_sink.rs` | Complete |
| Tests | Agent | Main | DB/service/live sink tests | Complete |
| Roadmap / handoff | Script | Main | roadmap + handoff | Complete |
| Migration/reliability review + gates | Skill | Main | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] Inputs and outputs are explicit
- [x] Same hash can exist in different banks
- [x] Same bank still dedupes
- [x] Legacy global index is dropped idempotently
- [x] Focused tests added
- [x] Migration/reliability review completed; P2 `mine_path_convos` coverage gap fixed
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
