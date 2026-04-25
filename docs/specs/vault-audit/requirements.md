# Requirements: Vault Audit

## Goal

- Provide a read-only, repeatable audit of the active vault before any backfill
  or orphan cleanup.

## Plain-Language Summary

- What this module does: counts what is really in the vault and explains what
  can be backfilled safely.
- Who it talks to: the filesystem under `/Users/mac-mini/Documents/wiki`.
- What user decision it implements: old orphan reports are evidence, not truth;
  fresh audit drives later work.

## Functional Requirements

- Read vault Markdown without modifying vault files, `wiki.db`, or `palace.db`.
- Count total files, Markdown files, source files, page files, root files,
  reports, and `.wiki/` artifacts.
- Count sources by origin.
- Count source frontmatter coverage, `compiled_to_wiki`, `source_id`,
  `notion_uuid`, required fields, and duplicate identity candidates.
- Count pages by directory, `entry_type`, status, `page_id`, `notion_uuid`, and
  unsupported/missing entry types.
- Recompute orphan categories from current files instead of trusting existing
  `.wiki/orphan-audit.json`.
- Output JSON and Markdown reports under vault `reports/`.
- Include backfill readiness categories:
  - ready source
  - ready page
  - missing stable ID
  - duplicate identity candidate
  - unsupported frontmatter
  - orphan candidate

## Non-Goals

- Do not write stable IDs.
- Do not backfill `wiki.db`.
- Do not initialize `palace.db`.
- Do not rewrite links or flip `compiled_to_wiki`.

## Inputs / Outputs

- Input: `/Users/mac-mini/Documents/wiki`.
- Output: audit JSON and Markdown reports in `<vault>/reports/`.

## Acceptance Criteria

- Audit is read-only.
- Report includes source/page/frontmatter/entry_type/compiled/orphan/ID stats.
- Report distinguishes old audit files from freshly computed results.
- Report is deterministic enough for review and follow-up planning.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: orphan cleanup policy after report review.
- Agent can automate: audit command, reports, tests, focused review.
