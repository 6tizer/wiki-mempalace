# Production Wiki Compiler Handoff

## Status

Implementation is complete pending PR and CI.

## Scope

This branch upgrades `batch-ingest` from the old summary-focused flow into a
local Wiki Compiler aligned with the user's Notion Wiki Compiler Instructions:

- one source -> one summary page,
- important ideas -> visible concept pages,
- concrete products/projects/tools/people -> visible entity pages,
- summary links to concept/entity pages,
- concept/entity pages link back to the summary,
- page writes emit `PageWritten` for Mempalace consumption.

## Main Changes

- `crates/wiki-core/src/llm_ingest_plan.rs`
  - Adds richer summary/concepts/entities plan fields.
  - Preserves backwards compatibility with old `LlmIngestPlanV1` JSON.
- `crates/wiki-cli/src/llm.rs`
  - Updates the LLM system prompt to request summary + concepts + entities.
- `crates/wiki-cli/src/wiki_compiler.rs`
  - New compiler runner.
  - Scans `compiled_to_wiki: false` sources.
  - Supports `--origin`, `--source-path`, and `--scope`.
  - Reuses frontmatter `source_id` when it exists in `wiki.db`; does not create
    duplicate DB sources for projected Notion source files.
  - Materializes summary/concept/entity pages and deduplicates references.
  - Parses source `tags:` from inline lists and YAML block lists.
  - Writes `source_id` before downstream page writes and marks local sources
    compiled only after page projection/report succeeds.
  - Reuses an existing DB source by URI + scope if frontmatter `source_id` is
    missing after an interrupted run.
- `crates/wiki-kernel/src/engine.rs`
  - Adds `write_page`, which inserts/replaces a page and emits `PageWritten`.
- `crates/wiki-kernel/src/wiki_writer.rs`
  - Disambiguates same-title page filenames with page id suffixes.
  - Removes obsolete managed page paths after projection.
- `docs/vault-standards.md`
  - Documents concept/entity page contract and bidirectional references.

## Verification

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
- Focused review findings fixed:
  - duplicate title summary projection overwrite,
  - YAML list source tags,
  - relationship scope filtering,
  - rich summary confidence metadata.
  - interrupted rerun raw-source duplication risk,
  - stale `--scope` help text.
- Production read-only dry-run:
  - X: `batch-ingest --origin x --limit 1 --dry-run` found 122 uncompiled sources and selected one 7733-character source.
  - WeChat: `batch-ingest --origin wechat --limit 1 --dry-run` found 66 uncompiled sources and selected one 41426-character source.

## Production Apply Boundary

No production write has been run on this branch. Only read-only dry-runs were
run against `/Users/mac-mini/Documents/wiki`.

Before tiny sample apply:

1. Back up `/Users/mac-mini/Documents/wiki`.
2. Confirm selected source paths.
3. Run one X source and one WeChat source with `--scope shared:wiki`.
4. Consume outbox to `/Users/mac-mini/Documents/wiki/.wiki/palace.db`.
5. Run `query` and `query/explain --palace-db`.
6. User checks Obsidian output before scaling.
