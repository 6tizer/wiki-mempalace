# Production Wiki Compiler Handoff

## Status

Implementation merged in PR #44. PR #46 ran the first backed-up production tiny
sample and added safety fixes. Broad scale-up is blocked on compiler
canonicalization v2.

## Scope

PR #44 upgrades `batch-ingest` from the old summary-focused flow into a
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
  - PR #46 adds pre-write safety gates and sample-driven dedup improvements, but
    the long-term fix is a resolver layer rather than more hardcoded aliases.
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

No production write was run in PR #44. PR #46 did run a backed-up tiny sample
against `/Users/mac-mini/Documents/wiki` and then verified the same X/WeChat
samples on restored temporary vault copies.

Before broad scale-up:

1. Implement compiler canonicalization v2.
2. Keep the resolver between compiler draft parsing and DB writes.
3. Retrieve bounded existing page candidates from `wiki.db`; do not load the
   whole wiki into the prompt.
4. Use a small LLM only for ambiguous candidate pairs.
5. Persist accepted alias/canonical decisions as data.
6. Keep Lint/Fixer as post-write governance: apply to DB, project Vault, sync
   Mempalace, then audit again.
7. Re-run X/WeChat regression samples before compiling more sources.
