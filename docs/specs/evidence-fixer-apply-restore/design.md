# Design: Evidence Fixer Apply + Restore

## Shape

- `wiki-core`: apply report, action report, tombstone, restore report contracts.
- `wiki-kernel`: pure-ish executor over `LlmWikiEngine` that mutates only when
  `apply=true`.
- `wiki-cli`: command wiring, writer lease, persistence, projection sync, report
  and tombstone file writing.

## Apply Flow

```text
plan.json
-> recheck action readiness and current subject state
-> preflight or mutate engine
-> save_to_repo_and_flush_outbox only if applied > 0
-> optional Vault projection sync
-> report JSON/Markdown + tombstone JSON files
```

The executor does not touch `palace.db`.

## Tombstones

Tombstone JSON includes:

- `tombstone_id`
- `action_id`
- subject type/id
- timestamp
- full page or claim snapshot

`restore` reads one tombstone file and replays the snapshot through engine write
methods, so normal DB save / outbox / projection paths are used.

## Recheck Rules

- Page/claim must still exist.
- Subject must be visible to the active `--viewer-scope`.
- Promotion uses `promote_page(..., force=false)` to re-run lifecycle rules.
- Semantic patch line range must still match `old_text`.
- Near duplicate merge is rejected unless the ready action contains web
  cross-verification evidence.
