# Design: xAI Web Search Provider

## Runtime

`kind = "xai"` maps to:

```text
POST https://api.x.ai/v1/responses
Authorization: Bearer $XAI_API_KEY
```

The request uses the configured `model`, defaulting to `grok-4.3`, and sends
Responses API tools:

```json
[
  {"type": "web_search"},
  {"type": "x_search"}
]
```

`tools` is configurable but constrained to `web_search` and `x_search`. The
adapter rejects `code_interpreter` because the local web-search runtime only
needs cited source discovery, not remote code execution.

## Evidence Mapping

xAI returns source URLs through response citations and output annotations. The
adapter prefers output annotations because they are tied to the answer text, and
falls back to top-level citations when annotations are absent. Each citation URL
maps to the existing `WebSearchEvidence` fields:

```text
query, provider, title, url, domain, snippet, summary, retrieved_at, content_hash
```

The model answer becomes snippet/summary context. The cited URLs remain the
dedupe and domain-count truth for cross-provider verification.

## Compatibility

The Tavily adapter is left in place for old local configs. Only the example
config, specs, and default policy wording move to Exa + xAI.
