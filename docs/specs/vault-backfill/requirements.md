# Requirements: Vault Backfill

## Goal

- Backfill existing vault sources/pages into vault-local `wiki.db` with stable
  IDs, outbox, dry-run, limit, and idempotency.

## Plain-Language Summary

- What this module does: gives historical files stable identities and imports
  them into the real runtime database.
- Who it talks to: vault Markdown, `wiki-storage`, `wiki-kernel`, and
  `wiki-cli`.
- What user decision it implements: `source_id` / `page_id` become system
  identity; `notion_uuid` remains provenance.

## Functional Requirements

- Support `--dry-run`, `--limit`, explicit vault path, target `wiki.db`, report
  output, and default scope.
- Default target db:
  `/Users/mac-mini/Documents/wiki/.wiki/wiki.db`.
- Default scope: `shared:wiki`.
- Add missing `source_id` to source files and missing `page_id` to page files
  through approved apply mode.
- Preserve existing `notion_uuid`, tags, source metadata, page status, and page
  `entry_type`.
- Import existing source records as `RawArtifact` records.
- Import existing summary/concept/entity/synthesis/qa/index/lint-report pages
  as `WikiPage` records.
- Generate outbox events needed by consumers.
- Repeated runs must not create duplicate logical source/page records.
- First version must not infer complex claims from existing Markdown.
- First version must not make source full text a default mempalace input.

## Non-Goals

- Do not rerun LLM over all historical sources.
- Do not solve orphan cleanup.
- Do not hand-edit SQLite.
- Do not change vault layout rules.

## Inputs / Outputs

- Input: B1 audit report and vault Markdown.
- Output:
  - stable IDs in frontmatter or an approved equivalent persistence path
  - populated `/Users/mac-mini/Documents/wiki/.wiki/wiki.db`
  - backfill JSON/Markdown reports
  - wiki outbox events

## Acceptance Criteria

- Dry-run reports all planned mutations without changing files or DB.
- Apply mode can be interrupted and rerun.
- Repeated apply does not duplicate logical records.
- Imported `wiki.db` has nonzero sources/pages from the vault.
- Outbox has events usable by B3.
- Full source bodies are not marked for palace default ingestion.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: full-vault apply after dry-run report.
- Agent can automate: implementation, fixture tests, limited dry-run, focused
  review.
