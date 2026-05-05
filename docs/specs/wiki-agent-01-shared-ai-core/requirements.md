# Requirements: wiki-agent PR1 Shared AI Core

## Goal

Create a reusable AI/search foundation for `wiki-agent` without changing current
`wiki-cli` behavior.

## Functional Requirements

- Add a new `wiki-ai` crate.
- Move reusable LLM profile loading, LLM request helpers, embedding helpers, and
  web search runtime into `wiki-ai`.
- Keep `wiki-cli` imports stable by re-exporting the moved modules from
  `crates/wiki-cli/src/llm.rs` and `crates/wiki-cli/src/web_search.rs`.
- Preserve `[llm]`, `[llm_profiles.*]`, `[embed]`, and `[web_search.*]` config
  behavior.
- Keep Exa, Tavily legacy support, and xAI provider behavior.
- Add default agent profile examples to `llm-config.example.toml`.

## Compatibility

- Existing `wiki-cli ai-profile`, `web-search`, `ingest-llm`, embeddings,
  synthesis, fixer, compiler, and MCP LLM paths must keep compiling and
  behaving the same.
- No command-line interface changes in PR1.
- No production data writes.

## Acceptance

- `cargo test -p wiki-ai` passes.
- `cargo test -p wiki-cli` passes.
- Workspace fmt/test/clippy passes before merge.
- No secret values are added to config examples.
