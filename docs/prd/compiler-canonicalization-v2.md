# PRD: Compiler Canonicalization v2

**Status**: active planning
**Related**: `Production Wiki Compiler`, `DB/Vault/Palace Consistency Governance`, `Notion Wiki Agent Contract`

## Goal

Make the production compiler safe to scale beyond tiny samples by inserting a
pre-write canonical resolver:

```text
raw source
 -> compiler draft
 -> pre-write canonical resolver
 -> small LLM fallback only for ambiguous candidates
 -> write wiki.db
 -> project Vault
 -> consume Mempalace
 -> post-write lint/audit
```

## Background

PR #46 completed the production compiler safety fix, ran X and WeChat tiny
samples, and documented the Notion agent contract. The tiny samples proved the
compiler path works, but also showed prompt-only dedup is not enough. The same
concept can appear as `MCP connectors`, `MCP连接器`, or `MCP 协议`.

Continuing broad production compilation before resolver v2 would create
duplicate concept/entity pages. The next step is to resolve each compiler draft
concept/entity against bounded existing DB candidates before any page write.

## User Value

- Production compilation can continue without turning Obsidian into duplicate
  concept/entity clutter.
- The system stops bad low-confidence canonical decisions before DB writes.
- Confirmed aliases become durable data, not prompt bloat or one-off code.
- Vault and Mempalace remain projections of `wiki.db`, not separate fix targets.

## Scope

### Phase 1: Candidate Retrieval

- For each compiler draft concept/entity, retrieve only a bounded top-K set from
  `wiki.db`.
- Candidate sources include title keys, alias keys, wikilink text, and short
  content hints.
- Do not send all pages or all aliases into the compiler prompt.

### Phase 2: Alias/Canonical Mapping

- Persist confirmed alias -> canonical page decisions as data.
- Use the mapping before fuzzy matching and before LLM fallback.
- Stop using source-specific hardcoded alias lists as the primary dedup path.

### Phase 3: Pre-Write Resolver

- Normalize title keys deterministically.
- Match exact title/canonical/alias keys first.
- Allow concept/entity cross-type fallback only when one canonical existing page
  is clear.
- Stop or record review-required findings for unresolved low-confidence cases.
- Return canonical page IDs/titles before summary/concept/entity pages are
  materialized.

### Phase 4: Small LLM Fallback

- Use LLM only when deterministic resolver returns a small ambiguous candidate
  set.
- The LLM sees only the draft item plus top-K candidates with title, type,
  aliases, and short excerpts.
- The LLM answers whether the draft item and candidate refer to the same wiki
  page. It does not recompile the article.

### Phase 5: Post-Write Governance Contract

- Lint/Fixer remains after normal compiler writes.
- Any fixer apply must update `wiki.db`, then project Vault, then sync
  Mempalace, then rerun audit.
- No fixer path may directly patch generated Vault Markdown or `palace.db` as
  source of truth.

### Phase 6: Regression Smoke

- Use the already-run X and WeChat tiny samples as regression evidence.
- Run against temporary vault/DB fixtures by default.
- Do not write to the real `/Users/mac-mini/Documents/wiki` vault unless the
  user explicitly approves.

## Non-Goals

- No broad production compile in this PR.
- No full duplicate-merge fixer implementation unless needed for resolver tests.
- No archived Notion source retirement.
- No page taxonomy redesign.
- No direct Vault or `palace.db` repair.
- No all-wiki prompt, all-page prompt, or large hardcoded alias glossary.

## Success Criteria

- Existing hardcoded compiler alias cases are covered by persisted mapping or
  bounded candidate retrieval.
- Resolver picks existing canonical concept/entity pages before writes.
- Ambiguous candidate sets use the small LLM fallback contract.
- Low-confidence unresolved cases stop page creation instead of silently
  creating duplicates.
- Summary links use resolved canonical page titles.
- Tests prove concept/entity cross-type fallback, alias persistence, bounded
  candidate retrieval, and LLM fallback parsing.
- Regression smoke uses temporary vault/DB data and does not mutate the real
  production vault.

## Stop Conditions

- Resolver needs whole-wiki prompt context.
- Fix requires adding many source-specific aliases in code or main compiler
  prompt.
- Candidate retrieval is unbounded.
- Low-confidence resolver output still writes a new page.
- Tests require touching real production vault.
- Fixer proposal edits generated Markdown or `palace.db` directly.

## Open Questions

1. Alias mapping shape: prefer DB table plus snapshot exposure, or DB table only
   with compiler repository queries.
2. LLM fallback apply policy: persist only deterministic/exact matches in this
   PR, or also persist accepted LLM `same_page=true` decisions.
3. Review-required behavior: fail the current source hard, or skip the item and
   still write summary with warning.
