# Tasks: Audit v2 PR 09 CJK / Unicode Retrieval

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-09-cjk-unicode-retrieval/` | Complete |
| Unicode FTS query builder | Agent | Main | `crates/rust-mempalace/src/service.rs` | Complete |
| CJK LIKE fallback | Agent | Main | `crates/rust-mempalace/src/service.rs` | Complete |
| Unicode rerank / sparse embedding | Agent | Main | `crates/rust-mempalace/src/service.rs` | Complete |
| CJK e2e + unit tests | Agent | Main | `crates/rust-mempalace/tests/e2e_core.rs`, service tests | Complete |
| Retrieval quality review + gates | Main/Subagent | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] Empty token query no longer becomes `"memory"`
- [x] Unicode tokens are preserved for FTS
- [x] CJK fallback uses exact / bigram / trigram LIKE patterns
- [x] fallback respects wing/hall/room/bank filters
- [x] English FTS operator input remains quoted
- [x] Chinese e2e search passes
- [x] `cargo test -p rust-mempalace -- --nocapture`
- [x] Retrieval quality review complete; P1 fallback truncation finding fixed
- [x] `cargo fmt --all -- --check`
- [x] `git diff --check`
- [x] `cargo deny --all-features check advisories bans licenses sources`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
