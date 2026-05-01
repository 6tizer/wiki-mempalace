# Handover: xAI Web Search Provider

## Status

Implementation complete on branch `codex/xai-web-search-provider`.

## Scope

Replace the checked-in dual-search default from Exa/Tavily to Exa/xAI while
keeping Tavily compatible for old local configs.

## Key Changes

- Added `kind = "xai"` web search provider.
- xAI calls `https://api.x.ai/v1/responses` with `web_search` and `x_search`.
- xAI citations and annotations map into `WebSearchEvidence`.
- `llm-config.example.toml` now uses `EXA_API_KEY` + `XAI_API_KEY`.
- Docs now describe Exa/xAI as the default cross-verification pair.

## Validation

- `cargo test -p wiki-cli --bin wiki-cli web_search::tests -- --nocapture`
- `cargo test -p wiki-cli --test research_synthesis_compose -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
