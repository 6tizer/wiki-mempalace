# Requirements: Evidence Fixer Apply + Restore

## Behavior

- Add `wiki-cli governance fixer-apply --plan <plan.json> --policy evidence-auto`.
- The command defaults to preflight. Mutations require `--apply`.
- Add `wiki-cli governance restore --tombstone <tombstone.json>`.
- Restore also defaults to preflight and requires `--apply` for mutation.
- Apply consumes the typed `EvidenceFixerPlan` from PR3.
- Only `ready` actions can execute. Non-ready actions are blocked.
- Apply must recheck current DB state before every action. Missing/stale subjects
  are skipped or blocked, not blindly applied.
- Retire and semantic patch actions must create tombstones before mutation.
- Merge duplicate claim actions must preserve all source references on the
  canonical claim before removing duplicate claims.
- Retire removes the page from `wiki.db` and emits `PageDeleted`; restore writes
  the page back and emits `PageWritten`.
- No command directly writes `palace.db`; projection/Palace updates remain
  downstream consumers of DB/outbox.

## Supported First Actions

- `promote_status`
- `add_missing_section`
- `set_title_from_h1`
- `dedupe_source_reference`
- `merge_duplicate` for claim IDs
- `semantic_patch` for page line ranges
- `retire_page`

Other action kinds stay blocked under `evidence-auto`.

## Acceptance

- Preflight does not mutate DB.
- `--apply` saves DB and flushes outbox only when actions were applied.
- Stale plan subjects are skipped.
- Claim merge does not drop source refs.
- Semantic patch only changes the target line range.
- Retire writes tombstone and removes the page from queryable DB state.
- Restore from tombstone recovers the DB page and projection when `--sync-wiki`
  is enabled.
