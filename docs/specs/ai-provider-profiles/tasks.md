# Tasks: AI Provider Profiles

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| PRD/spec setup | Main | `docs/prd/`, `docs/specs/ai-provider-profiles/` | Complete |
| LLM profile resolver | Main | `crates/wiki-cli/src/llm.rs` | Complete |
| Web search runtime | Main | `crates/wiki-cli/src/web_search.rs` | Complete |
| Smoke CLI commands | Main | `crates/wiki-cli/src/main.rs`, `commands/dispatch.rs` | Complete |
| Example config/docs | Main | `llm-config.example.toml`, indexes | Complete |
| Focused tests | Main | `llm.rs`, `web_search.rs`, CLI smoke as feasible | Complete |
| Review + gates | Main | local commands | Complete |

## Verification

- `cargo test -p wiki-cli --bin wiki-cli llm_profile -- --nocapture`
- `cargo test -p wiki-cli --bin wiki-cli web_search::tests -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
