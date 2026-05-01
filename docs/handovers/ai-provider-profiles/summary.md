# Handover: AI Provider Profiles

## Status

Implementation complete on branch `codex/ai-provider-profiles`.

## Scope

PR1 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3.

This PR owns:

- LLM profile config and resolver.
- Original Exa/Tavily web search runtime. Follow-up
  `codex/xai-web-search-provider` changes the checked-in default to Exa/xAI
  while keeping Tavily legacy-compatible.
- `ai-profile smoke` and `web-search smoke`.
- Example config and spec docs.

## Validation

- `cargo test -p wiki-cli --bin wiki-cli llm_profile -- --nocapture`
- `cargo test -p wiki-cli --bin wiki-cli web_search::tests -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
