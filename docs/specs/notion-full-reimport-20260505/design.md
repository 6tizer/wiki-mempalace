# Design: Notion Full Reimport 2026-05-05

## Pipeline

```text
Notion ZIPs
-> extract to staging
-> wiki-migration-notion dry-run
-> wiki-migration-notion migrate
-> vault-backfill --apply
-> notion-sync-index-backfill --apply
-> palace-init
-> query/lint/consistency checks
-> production backup
-> whole-directory replacement
-> production smoke checks
```

## Scanner

`scan_dir_with_report()` returns both imported `RawPage` records and structured
`ScanSkip` records. The existing `scan_dir()` API stays as a compatibility
wrapper.

The scanner first finds the canonical database root for the requested library,
then scans only `max_depth(1)`. This keeps nested task pages and copied root
pages out of the import set.

Library filters:

- Wiki keeps records only when `类型` parses as a valid wiki `EntryType`.
- X keeps all direct database Markdown records.
- WeChat keeps records with either `来源` or `文章链接`.

Dry-run reports include total skipped count, skipped reasons, and sample paths.
JSONL mode also writes `scan-skipped.jsonl`.

## Writer

Locations are allocated before writing. Each output bucket keeps a
case-insensitive set of used names. Candidate order:

1. `<slug>.md`
2. `<slug>-<uuid8>.md`
3. `<slug>-<uuid>.md`

This prevents macOS from overwriting files such as
`Human-In-The-Loop.md` and `Human-in-the-Loop.md`.

## Vault Backfill

Page planning reads metadata from frontmatter and stores it in `PlannedPage`.
Apply builds the desired `WikiPage` with imported metadata. Existing DB pages
are updated when any preserved metadata differs.

Supported time formats:

- RFC3339.
- `YYYY年M月D日 HH:MM`.
- `YYYY/M/D H:MM (GMT+8)`.

If a non-empty time field cannot be parsed, the report records a warning. New
records fall back to `now`; updates keep existing values when the export field
is invalid.

## Production Replacement

Production replacement is a directory swap, not in-place deletion:

1. Assert no known wiki writer is running.
2. Copy `/Users/mac-mini/Documents/wiki` to a timestamped backup.
3. Build a fresh replacement directory from verified staging output.
4. Move old production aside and move the replacement into place.
5. Run production smoke checks.

If a smoke check fails, restore the timestamped backup.
