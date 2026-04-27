# Module Handoff: Compiler Canonicalization v2

## Summary

- Added independent PRD/spec trio for compiler canonicalization v2.
- Added durable compiler alias/canonical mappings in `wiki.db`.
- Hardened `LlmIngestPlanV1` parsing for model-returned `null` values in
  string/list fields covered by the compiler contract.
- Replaced compiler hardcoded alias fallback with pre-write resolver:
  normalized keys -> persisted aliases -> bounded top-K candidates -> cross-type
  exact fallback -> small LLM fallback -> machine deferred resolution.
- Added machine-readable compiler run JSON with `deferred_resolutions` so
  lint/fixer agents can consume ambiguous items in the next machine pass.
- Rendered unresolved `related_names` as plain text so compiler output does not
  create broken wikilinks before resolver confirmation.
- Focused review fixes:
  - final page snapshot, outbox, and alias mappings commit in one SQLite
    transaction,
  - alias loading uses one scope query instead of per-page queries,
  - LLM fallback decisions do not persist durable alias mappings.
- Kept Vault/Mempalace as projections; no production vault writes were run.

## Files Changed

| File | Change | Reason |
| --- | --- | --- |
| `docs/prd/compiler-canonicalization-v2.md` | New PRD | Independent scope source before production scale-up |
| `docs/specs/compiler-canonicalization-v2/*` | New spec trio | Implementation SSOT |
| `docs/prd/README.md` | Added PRD index entry | Discoverability |
| `docs/specs/README.md` | Added spec index entry | Discoverability |
| `docs/roadmap.md` | Marked module in progress | Current roadmap state |
| `crates/wiki-storage/src/lib.rs` | Added `wiki_canonical_alias` table and helpers | Persist alias/canonical decisions as data |
| `crates/wiki-cli/src/wiki_compiler.rs` | Added resolver, candidate ranking, LLM fallback parser, mapping load/persist, deferred JSON reports | Block duplicate concept/entity pages before DB writes |
| `crates/wiki-core/src/llm_ingest_plan.rs` | Accept `null` for compiler string/list fields | Match prompt contract and live model output |

## Public Interfaces

- `CanonicalAliasMapping`
- `SqliteRepository::upsert_canonical_alias`
- `SqliteRepository::find_canonical_alias`
- `SqliteRepository::list_canonical_aliases_for_pages`
- `batch-ingest` remains the public compiler entrypoint.

## Known Limits

- LLM fallback is available in the compiler path; tests cover parser/prompt
  behavior and temp-vault live compiler smoke covers one source compile.
- Full duplicate-merge fixer is not implemented in this PR; fixer apply order is
  documented as DB -> Vault -> Mempalace -> audit. Ambiguous compiler items now
  land in machine-readable `deferred_resolutions` for that later agent lane.
- Real `/Users/mac-mini/Documents/wiki` production apply was not run.

## Dependencies

- Added: none.
- Changed: SQLite opens now create `wiki_canonical_alias`.
- Alternatives considered: continuing hardcoded aliases was rejected by PRD stop
  condition.

## Verification

- Commands:
  - `cargo fmt --all -- --check`
  - `cargo test -p wiki-storage` (18 tests)
  - `cargo test -p wiki-core llm_ingest_plan` (10 tests)
  - `cargo test -p wiki-cli wiki_compiler` (27 tests)
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `git diff --check`
- Temp-vault live smoke:
  - copied one WeChat source to `/tmp/wiki-compiler-v2-e2e-final.*`,
  - ran `batch-ingest` with temp DB/Vault,
  - ran `consume-to-mempalace` with temp palace,
  - ran `lint` against temp DB/Vault.
- Result: all passed; temp lint reported only `xref.missing` info and no broken
  wikilinks.

## Spec Status

- Requirements: implemented.
- Design: implemented.
- Tasks / checklist: implemented, reviewed, CI green, merged in PR #47.

## Next Notes

- Do not run broad production compile immediately. Next module should consume
  `deferred_resolutions` through resolver/lint/fixer agents, then use
  user-approved tiny production regression before scale-up.
