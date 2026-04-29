# Audit v2 PR 01 MCP Input Boundary Handoff

## Summary

Branch: `codex/audit-v2-01-mcp-input-boundary`

This PR implements Audit Report Follow-up v2 item 1:

- MCP result limits are clamped to `1..=100` in `wiki-cli` and standalone
  `rust-mempalace` MCP handlers.
- MCP tool schemas expose the same numeric bounds.
- `write_lint_report` rejects path traversal, absolute/nested names, hidden
  names, reports directory symlinks, and file-level symlink escapes.
- `docs/roadmap.md` now tracks the 11 Audit v2 PRs with a status column.

## Files Changed

- `crates/wiki-cli/src/mcp.rs`
- `crates/rust-mempalace/src/mcp.rs`
- `crates/wiki-kernel/src/wiki_writer.rs`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/specs/audit-v2-01-mcp-input-boundary/`
- `docs/handovers/audit-v2-01-mcp-input-boundary/summary.md`
- `docs/LESSONS.md`

## Interfaces

- MCP `wiki_query.per_stream_limit`, `mempalace_search.limit`, and
  `mempalace_reflect.search_limit` now clamp to `1..=100`.
- Non-integer MCP limits fail through existing tool-error paths.
- Lint report names must be slug filenames; accepted reports still write to
  `<wiki-root>/reports/<name>.md`.

## Known Limits

- Scope/capability hardening remains PR 02.
- Mempalace bank derivation remains PR 03.
- Service-level non-MCP callers are unchanged.

## Verification

Targeted checks already run:

- `cargo fmt --all -- --check`
- `cargo test -p wiki-cli mcp_numeric_limits_are_clamped_and_schema_bounded`
- `cargo test -p wiki-kernel lint_report_`
- `cargo test -p rust-mempalace numeric_limits_are_clamped`

Full workspace gate has passed:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`

Security-focused review found one P1 file-level symlink escape in lint report
writing; fixed with temp-file + rename write semantics and regression test.
