# Handoff: wiki-agent 05 Manager-Worker

## Scope

Branch: `codex/wiki-agent-manager-workers`.
PR: #117.

Implemented PR5 local draft:

- Manager prompt routing.
- Native worker runtime.
- MCP child fallback for MCP-exposed tools only.
- Writer lease guard for apply workers.
- Pure-read lint/search worker paths that do not record events.

## Verification

- `cargo test -p wiki-agent`
- `cargo fmt --all -- --check`
- `cargo test -p wiki-ai`
- `cargo test -p wiki-tools`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
- `git diff --check`
- After report-id cleanup: `cargo fmt --all -- --check`
- After report-id cleanup: `cargo test -p wiki-agent`
- After report-id cleanup: `cargo clippy -p wiki-agent --all-targets -- -D warnings`
- After report-id cleanup: `git diff --check`

## Notes

- `memory_curator` is intentionally routed but blocked until PR6.
- Governance/Fixer/Synthesis use kernel APIs under native backend.
- Lint/search do not use recorded MCP `wiki_lint`/`wiki_query` in native mode,
  because those paths mutate audit/query event state.
