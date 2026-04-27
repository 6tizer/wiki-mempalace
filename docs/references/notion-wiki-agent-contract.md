# Notion Wiki Agent Contract

This note records the Notion agent instructions that define the expected Wiki
compiler and maintenance loop. These pages are the behavior reference when the
local `wiki-mempalace` implementation differs from the older summary-only
pipeline.

## Source Pages

- Compiler: [Wiki Compiler Instructions](https://www.notion.so/Wiki-Compiler-Instructions-a3ce43893dce45658c219296b1045857)
- Lint: [Wiki Lint Agent Instructions](https://www.notion.so/Wiki-Lint-Agent-Instructions-516ec45b1a01426292b71ea5fd5b0d74)
- Fixer: [Wiki Fixer Instructions](https://www.notion.so/Wiki-Fixer-Instructions-ade5157acaf64cc78d02274640689fde)
- Reference upgrader: [引用升级员](https://www.notion.so/979843cff7d04e1bad4bcbe5b94fec40)

## Closed Loop

The Notion design is a four-agent loop, not a single best-effort compiler:

- **Compiler** reads a raw source article, deduplicates the summary, resolves
  existing concept/entity pages before creating new ones, then writes one
  summary plus concept/entity references.
- **Lint Agent** scans the whole Wiki for orphans, stale drafts, duplicate
  concept/entity names, duplicate summaries, status issues, tag issues, type
  mistakes, and unstructured references.
- **Fixer** consumes lint reports and applies safe fixes: status repair, tag
  repair, type migration, exact duplicate merge, clear near-duplicate merge,
  orphan reference repair, and structured-reference repair.
- **Reference upgrader** is a narrow historical migration worker. It upgrades
  plain-text `来源引用` rows in concept/entity pages into structured Notion
  `mention-page` links. It does not create, delete, or broadly rewrite pages.

## Compiler Requirements To Preserve Locally

- Summary dedup must run before creating pages:
  source page URL first, external article URL second, title match only as last
  fallback.
- Concept/entity dedup must run before creating pages:
  normalized exact match first, then conservative fuzzy/alias match.
- If an existing concept/entity is found, append a deduplicated source reference
  instead of creating a new page.
- If no existing page is found, classify the item:
  concept for methods, protocols, architectures, techniques, and non-obvious
  ideas; entity for concrete products, companies, people, projects, libraries,
  models, or tools.
- Summary pages should link to the resolved canonical concept/entity page names,
  not blindly to the names returned by the LLM.
- New summary pages are `approved`; new concept/entity pages are `draft`.
- Source tags are copied separately as source tags; Wiki tags follow the
  three-dimensional tag policy.

## Local Mapping

- Notion `mention-page` links map to Obsidian `[[wikilink]]` in vault
  projection.
- `wiki.db` remains the source of truth. Vault Markdown and Mempalace are
  projections.
- Notion mainly works as a write-then-lint/fix loop. The local system needs one
  extra pre-write layer because `wiki.db` writes also drive Vault projection,
  outbox, and Mempalace. Duplicates should be blocked before DB writes when
  possible.
- The local replacement for Notion's ad hoc page search is a canonical resolver
  between compiler draft parsing and DB writes. It should use normalized title
  keys, existing page candidates, persisted aliases, type checks, and bounded
  top-K retrieval from `wiki.db`.
- A small LLM fallback may judge ambiguous candidate pairs, but it receives only
  the draft item and retrieved candidates. It does not receive the whole wiki
  and does not recompile the article.
- Lint/Fixer stays post-write. When it repairs duplicates or references, it must
  apply to `wiki.db`, then project Vault, sync Mempalace, and audit again. It
  must not directly edit generated Vault Markdown or `palace.db` as the source
  of truth.
- The long-term local system should not need a recurring reference-upgrader
  agent. References should be structured in `wiki.db` first and rendered into
  vault links. A one-time migration worker may still be useful for historical
  Markdown debt.
