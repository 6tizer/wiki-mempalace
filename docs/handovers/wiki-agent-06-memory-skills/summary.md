# Handoff: wiki-agent 06 Memory + Skills

## Scope

Branch: `codex/wiki-agent-memory-skills`.
PR: [#118](https://github.com/6tizer/wiki-mempalace/pull/118).

Implemented:

- `EntryType::Skill`
- `pages/skill/` vault projection and vault-backfill import.
- Skill page eligibility for Mempalace live sink.
- explicit durable memory extraction from user markers: `记住：` / `remember:` / `技能：` / `skill:`.
- safe verifier for credentials, prompt-injection markers, invisible controls, oversized content.
- exact duplicate guard against existing concept/skill page markdown.
- `wiki-agent memory curate|status|search`.
- chat close auto-curation; no output when no candidates.
- PR5 `memory_curator` worker routed to the same curator.

## Review

- Fixed dry-run reporting so candidates are counted as `would_write`, not `written`.
- Fixed dry-run behavior so a missing wiki DB is not created just to check duplicates.
- Added Skill mappings across CLI/backfill/compiler/Notion export writer string/path helpers.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p wiki-ai`
- `cargo test -p wiki-tools`
- `cargo test -p wiki-agent`
- `cargo test -p wiki-cli`
- `cargo test -p wiki-migration-notion`
- `cargo test -p wiki-cli --test vault_backfill`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
- `git diff --check`
- After self-review dry-run fix: `cargo test -p wiki-agent`, `cargo clippy -p wiki-agent --all-targets -- -D warnings`, `git diff --check`
