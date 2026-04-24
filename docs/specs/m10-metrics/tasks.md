# Tasks: M10 Metrics Core

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Plan approved
- [x] Branch created
- [x] Subagent tasks assigned
- [x] Implementation complete
- [x] Module review complete
- [x] Tests added/updated
- [x] Docs updated
- [x] Integration review complete
- [ ] PR opened
- [ ] Codex/GitHub review addressed
- [ ] CI green
- [ ] Merged
- [x] Roadmap/PRD updated

## Subtasks

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Define metrics structs | Subagent | `crates/wiki-core/` | Done |
| Aggregate metrics | Subagent | `crates/wiki-kernel/`, `crates/wiki-storage/` | Done |
| CLI render/output | Subagent | `crates/wiki-cli/src/main.rs` | Done |
| Tests/docs | Subagent | tests, docs | Done |

## Review Notes

- 2026-04-24: M10 实现进入 In Review。已看到 `wiki-cli metrics`、`--json`、`--report <PATH>`、`--consumer-tag`、`--low-coverage-threshold`，以及 core/kernel/cli metrics 测试补强。
- 2026-04-24: Integration review 完成；P1 为新增文件未跟踪，随提交纳入分支解决。补充 CLI no-side-effect / custom consumer backlog 测试。
- PR、CI、merge 尚未完成。

## Verification

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
