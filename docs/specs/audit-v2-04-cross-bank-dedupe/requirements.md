# Requirements: Audit v2 PR 04 Cross-Bank Drawer Dedupe

## Goal

Allow identical drawer content to exist independently in different mempalace
banks while preserving duplicate suppression inside the same bank.

## Functional Requirements

- Drawer uniqueness must be `(bank_id, content_hash)`, not global
  `content_hash`.
- Existing palace databases must migrate idempotently:
  - drop the legacy global `idx_drawers_hash` index if present;
  - create `idx_drawers_bank_hash` on `(bank_id, content_hash)`.
- `mine_path` and `mine_path_convos` must check duplicates within the target
  bank only.
- `LiveMempalaceSink` must check duplicates within the sink bank only.
- Same content and source path in different banks must create one drawer per
  bank.
- Re-running the same import/sink in the same bank must remain idempotent.

## Non-Goals

- Do not change `content_hash` calculation.
- Do not run production palace migrations.
- Do not change KG/tunnel bank routing; that is PR 03.

## Acceptance

- Same `(content_hash, different bank_id)` rows can coexist.
- Duplicate `(bank_id, content_hash)` rows are rejected by SQLite and skipped by
  ingest/sink paths.
- Migration can run on old DBs with legacy `idx_drawers_hash`.
- Focused tests and workspace fmt/test/clippy pass.
