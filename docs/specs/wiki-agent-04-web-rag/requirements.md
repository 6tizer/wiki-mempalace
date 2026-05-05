# wiki-agent 04 Web RAG Requirements

## Scope

PR4 adds local + web evidence collection for `wiki-agent chat`.

## Functional Requirements

- `chat` collects internal evidence through native `wiki_query`.
- `--web off|auto|always` controls external search.
- `auto` only searches for freshness/current/external-looking prompts.
- Private viewer scopes do not run web search unless `--allow-private-web-search` is explicit.
- Shared-scope web search uses existing Exa/xAI provider config from `llm-config.toml`.
- Fake web evidence can drive tests without provider keys.
- Chat output clearly labels internal evidence and web evidence before the assistant answer.

## Non-Goals

- No full tool-call planner.
- No sub-agent search worker.
- No citation-aware final-answer verifier beyond cross-verified web evidence checks.

## Acceptance

- `--web off` does not use supplied fake web evidence.
- `--web always` under `shared:wiki` includes fake web evidence.
- `private:*` blocks web by default.
- `wiki-agent` focused tests pass without live web keys.
