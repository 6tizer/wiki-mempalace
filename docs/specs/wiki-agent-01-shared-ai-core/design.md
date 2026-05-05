# Design: wiki-agent PR1 Shared AI Core

## Design

`wiki-ai` owns shared AI infrastructure:

- `wiki_ai::llm`: app config loading, profile resolution, LLM chat helpers,
  embedding helpers, redaction, JSON slicing, and provider body overlays.
- `wiki_ai::web_search`: web provider config resolution, Exa/Tavily/xAI calls,
  cross-provider evidence runs, citation extraction, dedupe, and evidence hashes.

`wiki-cli` keeps source compatibility with thin re-export modules:

- `crate::llm::*` re-exports `wiki_ai::llm::*`.
- `crate::web_search::*` re-exports `wiki_ai::web_search::*`.

This keeps PR1 mechanical and avoids rewriting the existing CLI command surface.
Later PRs can depend on `wiki-ai` directly.

## Public API

- `wiki_ai::llm::load_app_config`
- `wiki_ai::llm::load_llm_profile_config`
- `wiki_ai::llm::resolve_llm_profile`
- `wiki_ai::llm::complete_chat`
- `wiki_ai::llm::complete_chat_json_object`
- `wiki_ai::llm::embed_first`
- `wiki_ai::web_search::run_search`

## Risks

- Moving tests into `wiki-ai` increases first build time because reqwest/mockito
  compile under the new crate. This is acceptable and only affects build cache.
- Keeping `wiki-cli` re-export modules avoids broad import churn.
