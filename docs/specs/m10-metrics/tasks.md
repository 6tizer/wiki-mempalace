# Tasks: M10 Metrics Core

## Checklist

- [ ] Requirements approved
- [ ] Design approved
- [ ] Plan approved
- [ ] Branch created
- [ ] Subagent tasks assigned
- [ ] Implementation complete
- [ ] Module review complete
- [ ] Tests added/updated
- [ ] Docs updated
- [ ] Integration review complete
- [ ] PR opened
- [ ] Codex/GitHub review addressed
- [ ] CI green
- [ ] Merged
- [ ] Roadmap/PRD updated

## Subtasks

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Define metrics structs | Subagent | `crates/wiki-core/` | Planned |
| Aggregate metrics | Subagent | `crates/wiki-kernel/`, `crates/wiki-storage/` | Planned |
| CLI render/output | Subagent | `crates/wiki-cli/src/main.rs` | Planned |
| Tests/docs | Subagent | tests, docs | Planned |

## Review Notes

-

## Verification

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
