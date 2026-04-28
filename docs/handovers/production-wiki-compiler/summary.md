# Production Wiki Compiler Handoff

## Status

Implementation merged in PR #44. PR #46 ran the first backed-up production tiny
sample and added safety fixes. PR #47 added compiler canonicalization v2. PR #54
added the deferred resolver/fixer lane. PR #56 fixed production boundary failures
and completed a full small-batch scale-up loop.

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
samples on restored temporary vault copies. PR #56 closed boundary hardening and
completed final small-batch production loops; 后续要求继续执行“无新功能混入、每批固定核对链路”。

Current operation loop:

1. Back up `/Users/mac-mini/Documents/wiki`.
2. Run `batch-ingest` with a small `--limit`.
3. Run `compiler-resolve-deferred --allow-create --apply` on the compiler run
   report.
4. Let apply project Vault, consume Mempalace, and write lint/audit reports.
5. Compare broken wikilinks and duplicate concept/entity groups against the
   pre-batch baseline.
6. Spot-check generated Vault pages before the next batch.

Latest production evidence:

- multiple 10-source real batches completed with `success=10` and `failed=0`;
- latest remaining uncompiled queue: 0 sources;
- latest deferred resolver apply created/aliased only through DB-first flow;
- follow-up deferred dry-run had no additional aliases/creates/changes to
  apply;
- no new broken wikilinks or duplicate concept/entity groups were introduced;
- `wiki.db` and `palace.db` integrity checks returned `ok`;
- latest consistency audit reported `vault_empty_unmanaged=0` and
  `palace_missing_page_drawers=0`.
