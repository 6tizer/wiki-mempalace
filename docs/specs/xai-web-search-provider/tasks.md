# Tasks: xAI Web Search Provider

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Config schema | Main | `crates/wiki-cli/src/llm.rs` | Complete |
| xAI adapter | Main | `crates/wiki-cli/src/web_search.rs` | Complete |
| Tests | Main | `web_search.rs`, `research_synthesis_compose.rs` | Complete |
| Example config | Main | `llm-config.example.toml` | Complete |
| Docs sync | Main | `docs/` | Complete |

## Validation

- `cargo test -p wiki-cli --bin wiki-cli web_search::tests -- --nocapture`
- `cargo test -p wiki-cli --test research_synthesis_compose -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
