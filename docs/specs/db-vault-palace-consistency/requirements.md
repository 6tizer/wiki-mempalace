# Requirements: DB/Vault/Palace Consistency Governance

## Goal

Create a repeatable loop that starts from `wiki.db`, compares the visible Vault
and derived Mempalace state, then applies only safe, auditable repairs.

## Functional Requirements

- `consistency-audit`
  - Accept `--db`, `--wiki-dir`, and optional `--palace`.
  - Load DB snapshot pages and sources as canonical records.
  - Scan only Vault `pages/` and `sources/` for content comparison.
  - Detect unmanaged empty Vault files, stale Notion-style links, unresolved
    local links, source-summary exact-match candidates, and compiled source
    graph-orphan candidates.
  - When `--palace` is provided, compare DB page IDs with Mempalace
    `wiki://page/<page_id>` drawers.
  - Report source drawers as out of scope, not missing, because source bodies do
    not enter Mempalace in this PR.
  - Write timestamped sibling reports under `<wiki-dir>/reports/`:
    `consistency-audit-<timestamp>.json` and
    `consistency-audit-<timestamp>.md`.
  - Markdown must be Chinese.

- `consistency-plan`
  - Accept `--audit-report <PATH>`.
  - Reject non-timestamped or missing audit reports.
  - Build a plan only from audit evidence.
  - Emit timestamped sibling reports:
    `consistency-plan-<timestamp>.json` and
    `consistency-plan-<timestamp>.md`.
  - Allow only these executable action classes:
    - `db_fix`: update DB page Markdown metadata/content through repository
      code paths.
    - `vault_cleanup`: remove unmanaged empty Vault files or approved stale
      report files.
    - `palace_replay`: replay DB page records into Mempalace through existing
      sink/replay code.
  - Non-executable findings may be reported as `needs_human` or `deferred`.

- `consistency-apply`
  - Accept `--plan <PATH>`.
  - Default to dry-run.
  - Require `--apply` for mutation.
  - Validate plan schema, audit timestamp, paths, action types, and whitelist
    before doing any write.
  - Apply order must be DB first, Vault projection second, Mempalace replay
    third.
  - Must not direct-write `palace.db`.
  - Must not run `batch-ingest`.
  - Must not delete any DB-known page/source path.

## Specific Findings To Handle

- Unmanaged zero-byte Vault files may be deleted only if DB has no matching
  page/source record.
- Old Notion export links in DB page Markdown may be removed or rewritten only
  by updating the DB record, then regenerating projection.
- Compiled source orphan candidates may receive a source reference only when a
  deterministic source-summary match exists by exact URL or title evidence.
- Mempalace page drift is repaired by replaying eligible DB pages through the
  same page sink used by `palace-init`.

## Acceptance Criteria

- Audit can be run on fixtures with all three layers and reports correct drift.
- Plan cannot invent paths or action types.
- Dry-run leaves DB, Vault, and Palace unchanged.
- Apply updates Vault only through projection after DB changes.
- Apply uses replay/sink for Mempalace page repairs and never manually mutates
  `palace.db`.
- Source bodies are not inserted into Mempalace.
