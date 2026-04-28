# PRD: Notion Archived Source Retirement

## Goal

When a Notion page that backs a local `notion://` source is archived or moved to trash, the local wiki must surface that state and retire the source through the DB-first pipeline.

## User Value

- Prevent archived Notion sources from staying silently active in `wiki.db`, Vault, and Mempalace.
- Keep production cleanup auditable and reversible.
- Preserve the boundary that `wiki.db` is the source of truth; Vault and Mempalace are projections.

## Scope

### PR 1: audit/plan

- Read existing `notion_page_index` rows.
- Retrieve Notion page `archived` / `in_trash` state.
- Generate a timestamped dry-run JSON/Markdown retirement plan.
- Do not mutate DB, Vault, or Mempalace.

### PR 2: apply

- Apply only unambiguous, `apply_safe=true` retirement actions.
- Mutate the DB origin first by removing the source and matching `notion_page_index` row in one SQLite transaction.
- Remove matching Vault source Markdown only by validated frontmatter identity.
- Keep manual-review cases out of apply.

## Non-Goals

- No manual Markdown deletion.
- No Notion write-back.
- No broad source deletion without DB evidence.
- No compiled page deletion.

## Acceptance

- `wiki-cli notion-archived-retirement plan` writes sibling JSON/Markdown reports.
- Fetch failures are reported, not hidden.
- Missing local source rows are reported as unsafe candidates.
- The plan can be reviewed before any apply PR.
