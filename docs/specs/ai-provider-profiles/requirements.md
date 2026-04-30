# Requirements: AI Provider Profiles

## Behavior

- Keep existing `[llm]` as the default LLM profile.
- Allow task-specific `[llm_profiles.<name>]` entries to inherit from the
  default profile and override provider URL, key env, model, temperature,
  reasoning effort, output limits, retries, and provider-specific request body.
- Keep provider URL allowlist and env-key precedence from existing LLM
  governance.
- Add web search provider config for Exa and Tavily.
- Allow one query to fan out to multiple configured providers and return a
  deduped evidence artifact.
- Add smoke commands:
  - `wiki-cli ai-profile smoke --profile <name>`
  - `wiki-cli web-search smoke --providers exa,tavily --query <query>`

## Compatibility

- Old `llm-config.toml` files with only `[llm]` and optional `[embed]` must keep
  working.
- Existing `ingest-llm`, embedding, `llm-smoke`, and MCP LLM paths keep using
  default `[llm]` unless future PRs opt into profiles.
- Missing web search keys must fail only when web search is explicitly invoked.

## Acceptance

- A named profile can inherit base URL/key from `[llm]` and override model and
  reasoning settings.
- A profile base URL outside `allowed_base_urls` is rejected.
- Exa/Tavily config resolves API keys from env or inline local fallback.
- Single provider missing key returns a clear blocked/error response without
  leaking secrets.
- Same URL from two providers is deduped while preserving provider provenance.
