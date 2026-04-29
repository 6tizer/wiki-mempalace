# Tasks: Audit v2 PR 06 LLM Governance

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-06-llm-governance/` | Complete |
| Config env keys + allowlist | Agent | Main | `crates/wiki-cli/src/llm.rs`, `llm-config.example.toml` | Complete |
| LLM request limits + redacted errors | Agent | Main | `crates/wiki-cli/src/llm.rs`, `wiki_compiler.rs` | Complete |
| Untrusted ingest prompt | Agent | Main | `crates/wiki-cli/src/llm.rs`, `main.rs`, `mcp.rs` | Complete |
| Structured JSON request path | Agent | Main | `main.rs`, `mcp.rs` | Complete |
| Focused tests | Agent | Main | `crates/wiki-cli/src/llm.rs` | Complete |
| Focused review + gates | Main | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] `api_key_env` preferred over inline key
- [x] Inline key remains fallback
- [x] Provider allowlist rejection exists
- [x] Input/output limit tests exist
- [x] Untrusted payload prompt redacts common secrets
- [x] JSON-object response requested for ingest LLM
- [x] Invalid output remains validated before mutation
- [x] Focused review complete; P2 response size gap fixed
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
