# Handoff: wiki-agent 04 Web RAG

## Scope

Branch: `codex/wiki-agent-web-rag`.

Implemented PR4 of the wiki-agent batch:

- Added `--web off|auto|always`.
- Added `--web-providers`, `--allow-private-web-search`, and hidden fake web evidence input for tests.
- Chat now gathers internal evidence through native `wiki_query`.
- Chat blocks web search under `private:*` unless explicitly allowed.
- Chat can use existing `wiki-ai` Exa/xAI web search config for live shared-scope web evidence.
- Evidence is rendered before the assistant answer and injected into the LLM prompt.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p wiki-ai`
- `cargo test -p wiki-tools`
- `cargo test -p wiki-agent`
- `cargo test -p wiki-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
- `git diff --check`
- manual fake web smoke with `--viewer-scope shared:wiki --web always`

## Notes

- PR4 does not implement a general tool planner or search worker.
- Native `wiki_query` records `QueryServed`, matching existing query behavior.
- Web evidence must be cross-verified before it enters the LLM prompt.
