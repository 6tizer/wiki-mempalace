# Requirements: xAI Web Search Provider

## Behavior

- Keep existing Exa provider behavior unchanged.
- Add `kind = "xai"` for web search providers.
- Use xAI Responses API with server-side `web_search` and `x_search` tools by
  default.
- Convert xAI citation URLs into the existing `WebSearchEvidence` artifact
  shape.
- Replace the checked-in default cross-verify example from Exa/Tavily to
  Exa/xAI.
- Keep Tavily adapter support for old private configs.

## Compatibility

- Existing `[web_search.providers.tavily]` configs still work.
- Missing `XAI_API_KEY` must produce the same clear blocked/error path as other
  providers.
- xAI must not enable `code_interpreter`; this provider is only for search
  evidence.

## Acceptance

- `llm-config.example.toml` uses `EXA_API_KEY` + `XAI_API_KEY`.
- `wiki-cli web-search smoke --providers exa,xai --query <query>` is the
  documented default smoke shape.
- xAI citations parse from top-level `citations` and output annotations.
- Unsupported xAI tools are rejected before an API call.
