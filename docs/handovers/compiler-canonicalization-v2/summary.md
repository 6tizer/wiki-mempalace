# Module Handoff: Compiler Canonicalization v2

## Summary

- Added independent PRD/spec trio for compiler canonicalization v2.
- Added durable compiler alias/canonical mappings in `wiki.db`.
- Replaced compiler hardcoded alias fallback with pre-write resolver:
  normalized keys -> persisted aliases -> bounded top-K candidates -> cross-type
  exact fallback -> small LLM fallback -> review skip.
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
| `crates/wiki-cli/src/wiki_compiler.rs` | Added resolver, candidate ranking, LLM fallback parser, mapping load/persist | Block duplicate concept/entity pages before DB writes |

## Public Interfaces

- `CanonicalAliasMapping`
- `SqliteRepository::upsert_canonical_alias`
- `SqliteRepository::find_canonical_alias`
- `SqliteRepository::list_canonical_aliases_for_pages`
- `batch-ingest` remains the public compiler entrypoint.

## Known Limits

- LLM fallback is available in the compiler path, but tests cover parser/prompt
  behavior without live network calls.
- Full duplicate-merge fixer is not implemented in this PR; fixer apply order is
  documented as DB -> Vault -> Mempalace -> audit.
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
  - `cargo test -p wiki-cli wiki_compiler` (26 tests)
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `git diff --check`
- Result: all passed.

## Spec Status

- Requirements: implemented.
- Design: implemented.
- Tasks / checklist: ready for focused/integration review.

## Next Notes

- After merge, do not run broad production compile immediately. First run a
  user-approved tiny production regression or a temp-vault smoke using the X and
  WeChat sample shapes.
