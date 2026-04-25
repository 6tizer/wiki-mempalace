# Production Vault Backfill Handoff

## Summary

- Date: 2026-04-25.
- Branch: `codex/vault-backfill-production-run`.
- Production vault: `/Users/mac-mini/Documents/wiki`.
- Backup: `/Users/mac-mini/Documents/wiki-backups/wiki-20260425-225500`.
- Result: historical vault content was backfilled into `wiki.db`, then synchronized to `palace.db`.

## Files Changed

| File | Change | Reason |
| --- | --- | --- |
| `/Users/mac-mini/Documents/wiki/**/*.md` | Added `source_id` or `page_id` frontmatter to managed source/page files. | Give historical vault records stable engine IDs. |
| `/Users/mac-mini/Documents/wiki/.wiki/wiki.db` | Created engine state and outbox rows. | Register historical sources/pages in the wiki engine. |
| `/Users/mac-mini/Documents/wiki/.wiki/palace.db` | Created mempalace drawers for searchable page content. | Make historical pages available to mempalace fusion search. |
| `/Users/mac-mini/Documents/wiki/reports/*.json` and `*.md` | Wrote audit/backfill/palace reports. | Keep machine-readable and human-readable production evidence. |

## Public Interfaces

- No CLI interface changed.
- Runtime default remains `shared:wiki` for this production vault.
- Palace bank used for this run: `wiki`.

## Known Limits

- `kg_facts=0`; this backfill produced searchable drawers, not KG facts.
- `vault-audit` still reports 4 orphan candidates under non-managed areas and 12 unsupported-frontmatter files. These do not block backfill and should be handled by a separate orphan governance task.
- 5 pages still lack `status`, and 16 sources still lack `compiled_to_wiki`; they were accepted by readiness checks and did not block import.

## Verification

- Backup:
  - `source_files=4501`
  - `backup_files=4501`
  - `source_markdown=4486`
  - `backup_markdown=4486`
  - backup size: `28M`
- Backfill apply:
  - `sources_imported=1099`
  - `pages_imported=3376`
  - `source_id_writes_applied=1099`
  - `page_id_writes_applied=3376`
  - `page_written_events=3376`
  - `skipped=0`
- Engine DB:
  - `sources=1099`
  - `pages=3376`
  - `page_written=3376`
  - `query_served=2`
- Mempalace:
  - `palace_init seen=3377 dispatched=3376 ignored=1 filtered=0 unresolved=0 acked=3377`
  - final consumer progress: `mempalace|3378`
  - final outbox head: `3378`
  - drawers: `3325`
  - kg_facts: `0`
  - `query_ok=true`, `explain_ok=true`, `fusion_ok=true`
- Fusion smoke:
  - `query "DVN" --palace-db ...` returned both `page:*` and `mp_drawer:*` results.
  - `explain "DVN" --palace-db ...` showed mempalace BM25/vector candidates.

## Spec Status

- Requirements: production B1 audit/backfill/palace initialization complete.
- Design: no code design changes in this run.
- Tasks / checklist: remaining follow-up is orphan governance based on the new audit report.

## Next Notes

- Do not rerun full `vault-backfill --apply` unless intentionally doing an idempotency check; the production vault now has stable IDs.
- Next production-data task should be B5 orphan governance, not another backfill.
- If rollback is required, restore from `/Users/mac-mini/Documents/wiki-backups/wiki-20260425-225500`.
