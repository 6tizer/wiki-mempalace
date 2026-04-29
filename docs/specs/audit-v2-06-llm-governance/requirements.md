# Requirements: Audit v2 PR 06 LLM Governance

## Goal

Harden `wiki-cli` LLM usage without changing production data: prefer API keys
from environment variables, treat source text as untrusted payload, bound LLM
input/output, redact sensitive text in prompts/errors, and add provider
allowlist support.

## Functional Requirements

- `[llm] api_key_env` must be supported and preferred over inline `api_key`.
- Inline `api_key` remains supported as fallback for compatibility.
- `[embed] api_key_env` must be supported and preferred over inline embed key.
- `[llm] allowed_base_urls` must reject LLM and embedding provider URLs outside
  the configured allowlist when the list is non-empty.
- LLM calls must enforce:
  - `max_input_chars`
  - `max_response_chars`
  - `max_output_tokens`
- `ingest-llm` and MCP `wiki_ingest_llm` must build a prompt that labels source
  content as untrusted payload and must redact common secrets before sending the
  source body to the LLM.
- `ingest-llm` and MCP `wiki_ingest_llm` must request JSON-object responses and
  keep `LlmIngestPlanV1::validate_bounds()` before dry-run or mutation.
- Error messages that include provider text or model text must pass through the
  existing deterministic redaction helper.

## Non-Goals

- Do not run live LLM calls.
- Do not change the persisted `LlmIngestPlanV1` schema.
- Do not remove inline key support in this PR.
- Do not change production `/Users/mac-mini/Documents/wiki` data.

## Acceptance

- Env key priority and inline fallback tests pass.
- Provider allowlist rejection test passes.
- Adversarial/untrusted source prompt test passes.
- Oversized input/output tests pass before network calls.
- Invalid/malformed output still fails before mutation and redacts sensitive
  provider/model text.
- Full workspace fmt/test/clippy pass.
