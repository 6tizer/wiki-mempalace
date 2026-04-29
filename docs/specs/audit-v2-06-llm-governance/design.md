# Design: Audit v2 PR 06 LLM Governance

## Config

`LlmConfig` keeps compatibility with inline `api_key`, but loading resolves keys
in this order:

1. `api_key_env` if set and the environment variable exists with non-empty
   value.
2. Inline `api_key` fallback.
3. Error when neither produces a non-empty key.

`EmbedConfig` gets the same `api_key_env` behavior. If no embed key is set, it
continues to inherit the resolved LLM key.

`allowed_base_urls` is optional. When non-empty, LLM and embed `base_url` values
must match one listed prefix. Empty list preserves existing local configs.

## Limits

Default limits:

- `max_input_chars = 200000`
- `max_response_chars = 200000`
- `max_output_tokens = 16384`

Every chat call validates prompt size and requested output tokens before network
I/O. Chat and embedding responses are also redacted in error messages.

## Prompt Boundary

`build_ingest_llm_user_prompt(...)` owns the source prompt format for CLI and
MCP:

- source URI is marked as untrusted metadata;
- source body is wrapped in `<source_document>...</source_document>`;
- the prompt explicitly says the source is untrusted payload and must not alter
  task/schema/security/tool instructions;
- URI and body are passed through `redact_for_ingest` before prompt assembly.

`ingest_llm_system_prompt()` repeats the untrusted-payload instruction.

## Structured Output

`ingest-llm` and MCP `wiki_ingest_llm` now use
`complete_chat_json_object(...)`, preserving `LlmIngestPlanV1::validate_bounds()`
and tag preflight before any engine mutation.

## Compatibility

- Existing inline-key configs still load.
- Existing configs without `allowed_base_urls` still load.
- Existing compiler/orphan-governance callers keep using shared `complete_chat`
  but now benefit from common limits and redacted error text.
