# Handoff: Audit Report Hardening

## Branch

`codex/audit-report-hardening`

## Merge

PR #58 已于 2026-04-28 合入。

## Completed

- Added bounded MCP stdin reader with 10 MiB per-line cap.
- Added shared `LlmIngestPlanV1::validate_bounds()` and wired it into CLI + MCP ingest-LLM paths.
- Expanded ingest redaction for common API tokens and PII-like samples.
- Added `WikiRepository::append_outbox_batch()` and SQLite transactional implementation; engine flush now writes whole batches.
- Quoted `rust-mempalace` FTS query tokens.
- Added `PRAGMA integrity_check` to `automation health`.
- Documented single-writer safety in README / AGENTS.
- Added `docs/mcp-api-reference.md`.
- Added focused tests and ran workspace verification.

## Deferred

- Row-level replacement for `wiki_state` remains a storage migration project.
- ANN-backed `wiki_embedding` search remains `embedding-ann-index`.
- `wiki-cli/src/main.rs` module split needs separate refactor PRD.
- MCP typed error mapping remains a separate MCP cleanup.
- Multi-process advisory lock / writer lease remains a separate runtime hardening item.

## Verification

- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
