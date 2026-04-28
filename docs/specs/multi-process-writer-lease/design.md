# Design: Multi-process Writer Lease

## Lease File

For `path/to/wiki.db`, the lock path is:

```text
path/to/wiki.db.writer.lock
```

The file stores owner metadata:

```text
owner=<owner>
pid=<pid>
acquired_at=<RFC3339>
expires_at=<RFC3339>
```

Acquire uses `OpenOptions::create_new(true)` so only one process can create the file.

## Expiry

Default TTL is six hours. CLI may override with `WIKI_WRITER_LEASE_TTL_SECS`.

If the lock file exists and `expires_at` is still in the future, acquire returns `StorageError::WriterLeaseBusy`.
If `expires_at` is in the past, acquire removes the stale file and retries once.

Malformed lock files are treated as busy, not deleted.

## CLI Integration

`wiki-cli` classifies subcommands:

- writer: ingest, query recording, lint/gap persistence, promotion, maintenance, automation writer jobs, Notion sync apply, compiler deferred apply, outbox ack/consume, MCP.
- read-only: metrics, dashboard, suggest, export, audits, dry-run plan/report commands.

Writer commands acquire the lease before `LlmWikiEngine::load_from_repo()` to prevent stale snapshot overwrite.

## Compatibility

This is an advisory process guard. It does not replace SQLite transactions. Existing `BEGIN IMMEDIATE` transaction boundaries stay unchanged.
