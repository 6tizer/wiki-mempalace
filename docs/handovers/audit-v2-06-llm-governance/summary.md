# Audit v2 PR 06 LLM Governance Handoff

## Summary

Branch: `codex/audit-v2-06-llm-governance`

This PR implements Audit Report Follow-up v2 item 6:

- Adds `api_key_env` support for `[llm]` and `[embed]`.
- Keeps inline API keys as compatibility fallback.
- Adds optional `allowed_base_urls` provider allowlist.
- Adds shared LLM prompt/output/token limits.
- Redacts provider/model error text before returning it.
- Builds `ingest-llm` prompts with explicit untrusted payload boundaries and
  redacts common secrets before sending source content to the LLM.
- Uses JSON-object chat response mode for CLI/MCP structured ingest.

## Files Changed

- `crates/wiki-cli/src/llm.rs`
- `crates/wiki-cli/src/main.rs`
- `crates/wiki-cli/src/mcp.rs`
- `crates/wiki-cli/src/wiki_compiler.rs`
- `llm-config.example.toml`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/specs/audit-v2-06-llm-governance/`
- `docs/handovers/audit-v2-06-llm-governance/summary.md`
- `docs/LESSONS.md`

## Verification

Focused checks:

- `cargo test -p wiki-cli llm -- --nocapture`

Focused review found one P2: provider error bodies were redacted but not
size-limited before entering error strings. Fixed by applying
`max_response_chars` to chat and embedding provider raw response text before
redaction/logging. Final re-review found no P0/P1/P2.

Full required gates passed:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
