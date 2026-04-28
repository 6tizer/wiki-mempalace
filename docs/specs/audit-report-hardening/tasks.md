# Tasks: Audit Report Hardening

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/audit-report-hardening`
- [x] Implementation
- [x] Focused tests
- [x] Workspace tests
- [x] Clippy
- [x] Handoff
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| MCP line-size guard | Agent | Main | `crates/wiki-cli/src/mcp.rs` | Complete |
| LLM ingest plan bounds validation | Agent | Main | `crates/wiki-core/src/llm_ingest_plan.rs`, CLI/MCP callers | Complete |
| Redaction rule expansion | Agent | Main | `crates/wiki-core/src/privacy.rs` | Complete |
| Batch outbox append transaction | Agent | Main | `crates/wiki-storage/src/lib.rs`, `crates/wiki-kernel/src/engine.rs` | Complete |
| FTS query token quoting | Agent | Main | `crates/rust-mempalace/src/service.rs` | Complete |
| Automation integrity health | Agent | Main | `crates/wiki-storage/src/lib.rs`, `crates/wiki-cli/src/main.rs` | Complete |
| Multi-process writer warning | Script | Main | `README.md`, `AGENTS.md` | Complete |
| MCP API reference | Script | Main | `docs/mcp-api-reference.md`, docs indexes | Complete |
| Architecture deferrals recorded | Script | Main | PRD/spec/roadmap | Complete |

## Verification

- `cargo test -p wiki-core redacts_common_tokens_and_pii`
- `cargo test -p wiki-core validate_bounds_rejects_oversized_plan`
- `cargo test -p wiki-cli mcp::tests::read_line_limited_rejects_oversized_line`
- `cargo test -p rust-mempalace service::tests::fts_query_quotes_user_tokens`
- `cargo test -p wiki-storage snapshot_and_outbox_commit_in_one_transaction`
- `cargo test -p wiki-cli --test automation_run_daily automation_health_reports_db_integrity`
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- PR #58 已于 2026-04-28 合入。
