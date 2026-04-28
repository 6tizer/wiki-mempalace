# Tasks: Reliability Test Matrix

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/reliability-test-matrix`
- [x] Storage failure injection tests
- [x] Large smoke tests
- [x] MCP malformed tests
- [x] LLM malformed JSON tests
- [x] Focused tests
- [x] Workspace tests
- [x] Clippy
- [x] Handoff
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Add DB batch rollback test | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Add large snapshot smoke | Agent | Main | `crates/wiki-storage/src/lib.rs` | Complete |
| Add MCP malformed parse test | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| Add LLM bad JSON tests | Agent | Main | `crates/wiki-cli/src/llm.rs` | Complete |
| Backfill docs/roadmap/LESSONS | Script | Main | `docs/` | Complete |

## Verification

- `cargo test -p wiki-storage reliability -- --nocapture`
- `cargo test -p wiki-cli reliability -- --nocapture`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- PR #65 merged on 2026-04-28.
