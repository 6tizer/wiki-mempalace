# Design: Web-backed Synthesis Composer

## Shape

- `wiki-cli/src/research_synthesis.rs`: compose report contracts, internal
  evidence pack builder, fake artifact readers, LLM prompts, validation, page
  builder, and report rendering.
- `wiki-cli/src/main.rs`: command wiring for `research-synthesis compose` and
  `research-synthesis run`, writer lease classification, persistence, and
  projection sync.
- Existing PR1 LLM/web runtime is reused:
  - `synthesis_research` generates external search queries.
  - Exa/xAI policy performs cross-provider search.
  - `synthesis_writer` writes JSON draft.
  - `synthesis_verifier` approves or blocks the draft.

## Compose Flow

```text
candidate
-> internal evidence pack from wiki.db pages
-> web research unless internal-only
-> draft JSON from synthesis_writer or --draft-json
-> verifier JSON from synthesis_verifier or --verifier-json
-> citation/evidence validation
-> optional --apply page write
-> compose JSON/Markdown report
```

Writes go through `LlmWikiEngine` + `save_to_repo_and_flush_outbox`; Vault
projection is refreshed only when `--sync-wiki` is active. Palace remains a
downstream consumer.

## Blocking Rules

- Private scope + web research without explicit override is blocked before
  search or LLM generation.
- Web-required runs block unless every web run is cross-verified.
- Key findings must cite exact allowed citation IDs.
- Verifier rejection is blocking.
- Blocked reports are still emitted so automation can record why no page was
  written.

## Compatibility

- Existing manual `synthesis` command remains unchanged.
- Discovery remains read-only.
- Fake artifact flags are test/offline inputs; production flow omits them.
