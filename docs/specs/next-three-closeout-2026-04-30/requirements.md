# Next Three Closeout 2026-04-30 Requirements

## Functional Requirements

- `wiki-cli verify-row-state` must verify row-level `wiki_state_row` data against the legacy `wiki_state` blob.
- The command must not acquire a writer lease, load the engine, write snapshot state, write outbox, or write Vault/Palace files.
- The command must support `--json` for machine-readable evidence.
- Missing `wiki_state_row` must be reported as a clear validation failure.
- `vault-report-paths` docs must reflect merged PR #22, not an active branch.
- The stale remote branch `codex/vault-report-paths` must be removed after merge verification.
- The hardening scheduled lane observation must record workflow URL, event, conclusion, and lane-level job results.

## Acceptance Criteria

- Focused CLI tests cover matching rows, JSON output, missing rows, and legacy blob-only DBs.
- Focused storage row-level tests still pass.
- Production read-only validation has recorded output.
- Hardening schedule run has recorded output.
- Roadmap, PRD index, spec index, handoff, and lessons are synchronized.

## Non-Goals

- Do not run production write commands.
- Do not remove blob fallback while production DB lacks row-level state rows.
- Do not change hardening workflow behavior when the observed scheduled run is green.
