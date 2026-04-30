# Handoff: Web-backed Synthesis Composer

Implementation complete on branch `codex/web-backed-synthesis-composer`.

PR6 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3.

## Scope

- Added web-backed synthesis compose/run commands.
- Reused PR1 LLM profiles and web search runtime.
- Added fake artifact inputs for deterministic tests without network/model calls.
- Added citation validation and verifier blocking before page write.
- Kept write path DB-first and Palace-free.

## Main Files

- `crates/wiki-cli/src/research_synthesis.rs`
- `crates/wiki-cli/src/main.rs`
- `crates/wiki-cli/src/web_search.rs`
- `crates/wiki-cli/tests/research_synthesis_compose.rs`

## Verification

Focused tests passed locally:

- `cargo test -p wiki-cli --test research_synthesis_compose`
- `cargo test -p wiki-cli --bin wiki-cli research_synthesis`

Full local gates passed:

- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
