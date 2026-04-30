# Next Three Closeout 2026-04-30 Design

## Row-State Verification Command

Add a no-engine CLI command:

```bash
wiki-cli --db <PATH> verify-row-state [--json]
```

Execution path:

1. `commands::dispatch::maybe_run_without_engine` handles the command before runtime setup.
2. `SqliteRepository::open_read_only` opens the DB with `SQLITE_OPEN_READ_ONLY`.
3. `verify_row_state_matches_blob` compares row-level snapshot reconstruction with the legacy blob.
4. CLI prints text or JSON evidence and exits non-zero if fallback cannot be retired.

Failure states:

- `row_count == 0`: row-level state is absent; blob fallback is still required.
- `blob_present == false`: legacy fallback cannot be compared.
- `matches_blob != Some(true)`: row-level state diverges from blob.

Legacy DB handling:

- If `wiki_state_row` is missing, storage reports `row_count = 0`.
- If `wiki_state` is missing, storage reports `blob_present = false`.
- This keeps old production DBs readable without schema mutation.

## Docs / Ops Closeout

- Mark `vault-report-paths` as merged PR #22 in the spec index.
- Record deletion of remote branch `codex/vault-report-paths`.
- Record hardening workflow run `25150878853` as the first observed scheduled success:
  - URL: <https://github.com/6tizer/wiki-mempalace/actions/runs/25150878853>
  - Event: `schedule`
  - Conclusion: `success`
  - Lanes: `perf`, `cjk-retrieval`, `db-corruption`, `mcp-boundary`, `bank-scope`

## Production Result

Read-only validation against `/Users/mac-mini/Documents/wiki/.wiki/wiki.db` returned:

```text
row_state status=error rows=0 blob_present=true matches_blob=n/a
Error: "row-level state has no rows; blob fallback is still required"
```

Decision: blob fallback stays. A future retirement PR must first do a controlled production row-state migration/backfill after backup/dry-run proof.
