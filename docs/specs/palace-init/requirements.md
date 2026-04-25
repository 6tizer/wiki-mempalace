# Requirements: Palace Init

## Goal

- Build `/Users/mac-mini/Documents/wiki/.wiki/palace.db` from backfilled
  `wiki.db` outbox and verify shared mempalace retrieval.

## Plain-Language Summary

- What this module does: turns wiki outbox into a searchable palace database.
- Who it talks to: `wiki.db`, `wiki-mempalace-bridge`, `rust-mempalace`, and
  `wiki-cli query/explain`.
- What user decision it implements: palace consumes high-quality pages and
  claims by default, not raw source full text.

## Functional Requirements

- Consume from `/Users/mac-mini/Documents/wiki/.wiki/wiki.db`.
- Write to `/Users/mac-mini/Documents/wiki/.wiki/palace.db`.
- Use viewer scope `shared:wiki` and palace bank `wiki`.
- Be safe to rerun without duplicating logical palace records.
- Validate:
  - drawers exist for backfilled eligible page content
  - kg facts exist when claims exist
  - query with `--palace-db` works
  - explain with `--palace-db` works
  - fusion includes mempalace candidates where available
- Emit a JSON/Markdown init report.

## Non-Goals

- Do not consume full source bodies by default.
- Do not create new claims from Markdown.
- Do not change B2 backfill identity rules.

## Inputs / Outputs

- Input: backfilled `wiki.db` and outbox.
- Output: initialized `palace.db`, consumer progress, validation report.

## Acceptance Criteria

- Palace init can run from an empty missing palace path.
- Rerun is safe.
- Query/explain work with `shared:wiki`.
- Missing optional claims do not fail page drawer validation.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: policy change for source full-text ingestion.
- Agent can automate: implementation, fixture tests, validation report.
