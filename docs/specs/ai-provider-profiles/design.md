# Design: AI Provider Profiles

## LLM Profiles

`llm-config.toml` keeps `[llm]` as default. Optional
`[llm_profiles.<name>]` overlays are resolved at runtime:

```text
default [llm]
 -> optional parent profile
 -> requested profile override
 -> env key resolution
 -> allowed_base_urls validation
```

Profiles are intentionally passive in this PR. Existing commands keep using the
default profile; future Fixer/Synthesis commands choose task profiles.

Provider-specific JSON is carried through `extra_body` and merged into the
OpenAI-compatible chat body after default fields are set.

## Web Search Runtime

`[web_search.providers.<name>]` defines a provider. The first adapters are:

- Exa: `POST https://api.exa.ai/search`, `x-api-key` header.
- Tavily: `POST https://api.tavily.com/search`, bearer auth.

The runtime returns `WebSearchEvidence`:

```text
query, provider, title, url, domain, snippet, summary, retrieved_at, content_hash
```

Search fan-out is sequential in this PR to keep the blocking CLI simple. The
contract still accepts multiple providers for the same query and records
provider success/failure independently.

## Commands

Both smoke commands run through the no-engine dispatch path, so they do not open
`wiki.db` or acquire writer leases.
