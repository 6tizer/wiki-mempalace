# Next Three Closeout 2026-04-30 PRD

## Status

Completed by PR #99: <https://github.com/6tizer/wiki-mempalace/pull/99>

## Goal

Close the three immediate follow-ups after the audit disposition batch:

1. Clean stale `vault-report-paths` workflow/docs residue.
2. Run row-level state production validation without writing production data.
3. Observe the new scheduled hardening lane and record the result.

## Scope

- Update active docs so `vault-report-paths` no longer appears as an active branch after merged PR #22.
- Delete the stale remote branch `codex/vault-report-paths` after verifying PR #22 is merged.
- Add a read-only CLI validation path for row-level `wiki_state_row` vs legacy blob compatibility.
- Run the validation against `/Users/mac-mini/Documents/wiki/.wiki/wiki.db` without writing Vault, Palace, or DB state.
- Record whether blob fallback can be retired.
- Record the first real scheduled `Hardening` workflow result after PR #93.

## Out Of Scope

- No production DB migration or backfill.
- No removal of legacy blob fallback in this batch.
- No changes to hardening lane coverage unless the observed run fails.

## Acceptance

- `verify-row-state` opens the DB read-only and reports row/blob compatibility in text or JSON.
- Legacy blob-only DBs produce a clear validation failure instead of a raw SQLite table error.
- Production validation evidence is recorded in roadmap and handoff.
- Hardening scheduled run evidence is recorded in roadmap and handoff.
- `docs/specs/README.md` no longer lists `vault-report-paths` as active.
