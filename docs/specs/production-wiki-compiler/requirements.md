# Requirements: Production Wiki Compiler

## Goal

Implement the local Wiki Compiler so it matches the user's Notion Wiki Compiler
Instructions, then operate it through controlled production batches.

## Plain-Language Summary

- What this module does: turns one raw source article into one summary page plus
  visible concept/entity wiki entries.
- Who it talks to: `wiki.db`, source Markdown under `sources/{origin}/`, managed
  page projection under `pages/`, LLM config, outbox, and Mempalace consumer.
- What user decision it implements: the local system must match the Notion
  compiler outcome, not the old summary-only `batch-ingest` behavior.

## Functional Requirements

- The compiler must process only sources with `compiled_to_wiki: false` unless a
  future explicit recompile mode is added.
- Each successful article compile must normally create or update:
  - exactly one summary page,
  - 3-7 concept/entity entries when the article contains enough substance,
  - source references from concept/entity pages back to the summary.
- Summary pages must follow `docs/vault-standards.md`:
  - `entry_type: summary`,
  - `status: approved`,
  - `confidence: high|medium|low`,
  - five required body sections.
- Concept pages must use `entry_type: concept` and initial `status: draft`.
- Entity pages must use `entry_type: entity` and initial `status: draft`.
- Concept/entity extraction must distinguish:
  - concept: methodology, protocol, architecture pattern, technique, or
    non-obvious idea,
  - entity: concrete product, company, person, project, library, model, or tool.
- The compiler must deduplicate before creating pages:
  - summary dedup by source id / source URL / external article URL,
  - concept/entity dedup by normalized title and conservative fuzzy title match.
- The compiler must not rely on an unbounded prompt glossary or a growing list
  of one-off aliases for production deduplication.
- A pre-write canonical resolver must run after the LLM compiler draft and
  before any concept/entity page write:
  - deterministic normalization first,
  - existing page candidate retrieval from `wiki.db`,
  - alias/canonical mapping lookup,
  - concept/entity cross-type match check,
  - bounded top-K candidates only.
- Ambiguous canonical decisions may use a small LLM call, but that call must
  compare the draft item against retrieved candidates only. It must not re-read
  or recompile the full article.
- Confirmed alias/canonical decisions must be stored as data in
  `wiki_canonical_alias`, not only in code and not inside the main compiler
  prompt.
- Low-confidence canonical decisions must become machine-owned
  `deferred_resolutions`; they must not silently create duplicate pages or
  enter a human/manual lane.
- Existing concept/entity pages must be updated incrementally, not rewritten from
  scratch.
- References must be structured for Obsidian and system audit:
  - summary page lists extracted concepts/entities with wiki links,
  - concept/entity page has a `## 来源引用` section linking back to the summary,
  - frontmatter or body must retain machine-readable ids where available.
- Tags must follow the three-dimension policy from the Notion compiler
  instructions:
  - A: scenario/domain,
  - B: technical method,
  - C: product shape.
- Raw source tags must be preserved as source tags; compiler tags must be
  separate wiki tags.
- After a successful local compile, the source Markdown must be marked
  `compiled_to_wiki: true`.
- Notion writeback stays disabled unless explicitly enabled later.
- `batch-ingest` must remain the automation-compatible entrypoint, but its
  internals must use the new compiler contract.
- The first production run must support a tiny sample of one X source and one
  WeChat source.
- Lint/Fixer remains a post-write governance path. When it fixes duplicate or
  reference issues, it must apply to `wiki.db`, emit the needed outbox/page
  changes, re-project Vault, re-sync Mempalace, and then run audit again.

## Non-Goals

- No full 176-source production compile in the first PR.
- No archived source retirement.
- No broad schema redesign.
- No direct `palace.db` writes.
- No manual editing of generated Markdown as the fix path.
- No unattended hourly automation until repeated small production batches pass
  audit and Vault spot-checks.
- No unattended broad compile until repeated small batches pass post-run audit
  and Vault spot-checks.
- No direct Vault or `palace.db` patching from Lint/Fixer.
- No whole-wiki context dump into the compiler prompt.

## Inputs / Outputs

- Input:
  - source Markdown from `sources/x/` and `sources/wechat/`,
  - `wiki.db` snapshot,
  - `llm-config.toml`,
  - current `DomainSchema`.
- Output:
  - one summary page per compiled source,
  - concept/entity pages created or incrementally updated,
  - updated source `compiled_to_wiki`,
  - outbox events for written pages,
  - run report with counts and evidence.

## Acceptance Criteria

- A tiny sample with one X source and one WeChat source compiles successfully.
- Controlled small production batches compile successfully before any broad
  unattended run.
- Each compiled source has one summary page.
- Each non-trivial sample produces visible concept/entity pages or updates
  existing ones with deduplicated source references.
- Obsidian shows summary, concept/entity pages, and links clearly.
- Mempalace consumes the resulting page events.
- `query` and `query/explain --palace-db` can surface the compiled content.
- A run report records backup path, sample identity, commands, counts, and known
  issues.
- For scale-up batches, deferred resolver apply, lint/audit, duplicate checks,
  and Mempalace consume all complete before the next batch starts.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed:
  - spec approval before code,
  - production apply beyond controlled small batches,
  - scale decision after Obsidian review.
- Agent can automate:
  - implementation,
  - focused review,
  - dry-runs,
  - run report generation,
  - Mempalace consumption and query smoke.
