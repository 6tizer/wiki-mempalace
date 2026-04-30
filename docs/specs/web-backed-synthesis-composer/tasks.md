# Tasks: Web-backed Synthesis Composer

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| PRD/spec setup | Main | `docs/prd/`, `docs/specs/web-backed-synthesis-composer/` | Complete |
| Compose contracts + validation | Main | `crates/wiki-cli/src/research_synthesis.rs` | Complete |
| CLI commands | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Web evidence serde support | Main | `crates/wiki-cli/src/web_search.rs` | Complete |
| Fake web/LLM verifier tests | Main | `crates/wiki-cli/tests/research_synthesis_compose.rs` | Complete |
| Full gates + PR | Main | local commands / GitHub CI | Local complete; PR CI pending |

## Verification

- `cargo test -p wiki-cli --test research_synthesis_compose`
- `cargo test -p wiki-cli --bin wiki-cli research_synthesis`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`

All local gates passed before PR.
