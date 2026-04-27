# Requirements: Compiler Canonicalization v2

## Goal

Insert a pre-write resolver into the production compiler so each draft
concept/entity resolves to an existing canonical page when possible, using
bounded DB candidates and durable alias data before any Vault/Mempalace
projection.

## Plain-Language Summary

- Compiler reads one raw source and asks the LLM for a draft.
- Resolver checks each draft concept/entity against existing `wiki.db` pages.
- Clear match updates existing page. Clear miss creates one new page.
- Ambiguous match uses a small LLM pairwise/candidate-set judgment.
- Unsafe match stops the item or source instead of creating a duplicate.

## Functional Requirements

- The resolver must run after compiler draft parsing and before
  `materialize_compiler_pages` writes concept/entity pages.
- Candidate retrieval must be per draft item and bounded by top-K.
- Candidate retrieval must only read from `wiki.db`/loaded engine state.
- Candidate retrieval must consider:
  - normalized title keys,
  - compact title keys,
  - persisted alias/canonical mappings,
  - page title,
  - page entry type,
  - wikilink/body hints,
  - scope.
- Candidate retrieval must not scan the whole page list into the LLM prompt.
- Confirmed alias/canonical mappings must be stored as data, not hardcoded in
  compiler code.
- Resolver order must be:
  1. normalize requested title,
  2. lookup persisted aliases,
  3. exact candidate match within preferred type,
  4. exact candidate match across concept/entity when clearly canonical,
  5. conservative fuzzy/content candidate ranking,
  6. small LLM fallback for bounded ambiguous candidates,
  7. unresolved stop/review warning.
- Concept/entity cross-type fallback is allowed only for `concept` <-> `entity`;
  no other page type may be used as canonical target.
- Low-confidence unresolved candidates must not create duplicate pages.
- Existing concept/entity pages must receive deduplicated source references.
- Summary pages must link to resolved canonical titles, not raw LLM names.
- New pages must still use existing compiler page contracts and
  `docs/vault-standards.md`.
- LLM fallback must not receive the full article body or full wiki.
- LLM fallback output must parse into a deterministic decision:
  `same_page`, `canonical_title`, `confidence`, `reason`.
- Accepted alias decisions should be persisted when safe.
- Dry-run and tests must stay read-only.
- Regression smoke must use a temporary vault/DB fixture by default.
- Production vault apply requires explicit user approval.
- Post-write fixer governance must apply in this order:

  ```text
  lint/audit finding
   -> fixer plan
   -> apply to wiki.db
   -> project Vault
   -> consume Mempalace
   -> audit again
  ```

## Non-Goals

- No broad production compile.
- No direct generated Vault Markdown patching.
- No direct `palace.db` patching.
- No archived source retirement.
- No new taxonomy/page-type system.
- No all-wiki prompt.
- No large code-maintained alias table.

## Inputs / Outputs

- Input:
  - `LlmIngestPlanV1` draft concepts/entities,
  - existing `wiki.db` pages,
  - alias/canonical mapping data,
  - optional LLM config for ambiguous fallback,
  - source metadata for report warnings/deferred items.
- Output:
  - resolved page ID/title/type per draft item,
  - new-page decision for clear misses,
  - machine-owned deferred resolution item for ambiguous unsafe cases,
  - persisted alias/canonical mapping for confirmed safe aliases,
  - updated compiler run report and JSON artifact for lint/fixer consumption.

## Acceptance Criteria

- Existing sample aliases such as `MCP connectors`, `MCP连接器`, and `MCP 协议`
  resolve through data/candidates, not one-off compiler alias code.
- A draft concept can resolve to an existing entity when that entity is the clear
  canonical page.
- Multiple fuzzy candidates do not create a new duplicate page.
- Multiple fuzzy candidates produce `deferred_resolutions` machine data instead
  of prose-only review text.
- The small LLM fallback is invoked only for bounded ambiguous candidates.
- Summary `## 提取的概念` contains canonical wiki links after resolver.
- Unit tests cover normalizer, mapping lookup, candidate top-K, cross-type
  fallback, low-confidence stop, LLM fallback parsing, and source-reference
  dedup.
- Integration tests compile fake X/WeChat-like sources in a temporary vault and
  prove no duplicate concept/entity pages are created.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed:
  - production vault apply,
  - any PRD scope expansion,
  - broad compile scale decision.
- Agent can automate:
  - spec implementation,
  - subagent split,
  - temporary regression fixtures,
  - focused/integration review,
  - tests and docs updates.
