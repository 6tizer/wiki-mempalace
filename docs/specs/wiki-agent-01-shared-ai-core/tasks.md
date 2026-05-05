# Tasks: wiki-agent PR1 Shared AI Core

## Checklist

- [x] Create `wiki-ai` crate and workspace entry.
- [x] Move reusable LLM code into `wiki-ai::llm`.
- [x] Move reusable web search code into `wiki-ai::web_search`.
- [x] Re-export moved modules from `wiki-cli`.
- [x] Add agent profile examples to `llm-config.example.toml`.
- [x] Add PRD and spec trio.
- [x] Run focused tests.
- [x] Run workspace gate.
- [ ] Self-review and open PR.

## Review Focus

- `wiki-cli` behavior must remain unchanged.
- New crate must not leak API keys in errors or tests.
- xAI citation-required behavior must remain intact.

## Status

Implemented locally on branch `codex/wiki-agent-shared-ai-core`; PR pending.
