# Palace Init Bridge Handoff

## Scope

- Owner: Subagent C.
- Branch: `codex/vault-backfill-palace-init`.
- Files changed:
  - `crates/wiki-mempalace-bridge/src/lib.rs`
  - `crates/wiki-mempalace-bridge/src/live_sink.rs`
  - `crates/wiki-mempalace-bridge/tests/live_sink.rs`
  - `crates/wiki-cli/src/palace_init.rs`
  - `crates/wiki-cli/tests/palace_init.rs`

## Changes

- `MempalaceWikiSink` now has default no-op `on_page_written(&WikiPage)`.
- `OutboxResolver` can resolve `PageId -> WikiPage`; `PageWritten` events now resolve page scope, run `scope_filter`, then dispatch to the sink.
- `LiveMempalaceSink::on_page_written` writes one palace drawer:
  - `source_path = wiki://page/<uuid>`
  - `content = page.markdown`
  - `bank_id` from sink construction
  - only for summary / concept / entity / synthesis / qa pages
- `LiveMempalaceSink::on_source_ingested` is now no-op, so raw source events do not create source drawers.
- `crates/wiki-cli/src/palace_init.rs` provides core init helpers:
  - `mempalace_bank_from_viewer_scope`
  - `run_palace_init_core`
  - `run_live_palace_init`
- `palace-init` stops before ack if required events are unresolved.
- `palace-init` acks outbox only after palace write and validation both
  succeed.
- `palace-init` writes JSON/Markdown report files and runs a light
  query/explain/fusion smoke against `MempalaceSearchPorts`.

## Tests

- PageWritten creates a drawer.
- Re-running the same PageWritten event does not duplicate the drawer.
- SourceIngested does not create a drawer.
- `shared:wiki` maps to palace bank `wiki`.
- Core init consumes outbox, acks head, and reports dispatch/drawer stats.
- Core init does not ack unresolved required events.
- Live init does not ack when validation fails.
- Report writer outputs JSON and Markdown.
- Ineligible page types do not create drawers.

## Main-Agent Notes

- Main-agent integration added the `palace-init` CLI command and a page-aware
  `EngineResolver`.
- Existing `consume-to-mempalace` also now stops before ack when unresolved
  required events exist.

## Review Follow-up

- RC P1 unresolved events ack risk fixed in `run_palace_init_core` and
  `consume-to-mempalace`.
- RC P2 unknown supersede scope now counts unresolved instead of dispatching.
- RC P2 query/explain/fusion validation now appears in palace-init report.
- Integration P2 validation-before-ack fixed by moving ack after validation.
