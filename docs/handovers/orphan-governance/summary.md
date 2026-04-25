# Module Handoff: Orphan Governance / Implementation

## Summary

- Added read-only `wiki-cli orphan-governance`.
- Command consumes `vault-audit.json`, classifies B5 findings into governance lanes, and writes sibling JSON/Markdown reports.
- v1 mutation policy is explicit: reports only; no vault Markdown mutation; no DB/outbox/palace writes; no apply mode.

## Files Changed

| File | Change | Reason |
| --- | --- | --- |
| `crates/wiki-cli/src/orphan_governance.rs` | New report model, audit JSON parsing, lane classification, report-dir validation, JSON/Markdown rendering | Implements B5 read-only governance command behavior |
| `crates/wiki-cli/src/main.rs` | Added module, CLI subcommand, early handler before DB open | Wires `orphan-governance --audit-report <PATH> [--report-dir <PATH>]` |
| `crates/wiki-cli/tests/orphan_governance.rs` | New module tests | Covers 4/12/5/16 classification, output files, report-dir constraint, read-only sample markdown |
| `crates/wiki-cli/tests/vault_cli_commands.rs` | Added CLI smoke test | Verifies command writes both report files under `<wiki-dir>/reports` |
| `docs/handovers/orphan-governance/summary.md` | New handoff | Required subagent handoff |

## Public Interfaces

- CLI:

  ```bash
  cargo run -p wiki-cli -- \
    --wiki-dir /Users/mac-mini/Documents/wiki \
    orphan-governance \
    --audit-report /Users/mac-mini/Documents/wiki/reports/vault-audit.json
  ```

- Output:
  - `orphan-governance-report.json`
  - `orphan-governance-report.md`

- Report-dir rule:
  - With `--wiki-dir`: default is `<wiki-dir>/reports`; explicit `--report-dir` must resolve under `<wiki-dir>/reports`.
  - Without `--wiki-dir`: default is audit report parent.

## Known Limits

- No apply/dry-run mutation mode.
- Unsupported frontmatter, missing status, and missing `compiled_to_wiki` currently use count-only audit fields because `vault-audit.json` does not expose path arrays for those categories.
- Future auto-fix for missing page `status` requires spec update plus user approval.

## Dependencies

- Added: none.
- Changed: none.
- Alternatives considered: reuse `vault_audit` structs directly; rejected because governance consumes persisted JSON and needs only stable report fields.

## Verification

- Commands:
  - `cargo fmt --all -- --check`
  - `cargo test -p wiki-cli --test orphan_governance`
  - `cargo test -p wiki-cli --test vault_cli_commands`
  - `cargo run -p wiki-cli -- --wiki-dir /Users/mac-mini/Documents/wiki orphan-governance --audit-report /Users/mac-mini/Documents/wiki/reports/vault-audit.json`
- Result:
  - Format check passed.
  - `orphan_governance`: 6 passed.
  - `vault_cli_commands`: 4 passed.
  - Production audit command reported `orphan_candidates=4 unsupported_frontmatter=12 pages_missing_status=5 sources_missing_compiled_to_wiki=16` and wrote reports under `/Users/mac-mini/Documents/wiki/reports/`.

## Review Follow-up

- P2 malformed/old audit report fixed: required audit fields now fail fast
  instead of becoming zero-count governance reports.
- P2 report-dir symlink escape fixed: with `--wiki-dir`, validation checks the
  canonical nearest existing ancestor and rejects writes that escape
  `<wiki-dir>/reports`.
- P3 task status fixed in `docs/specs/orphan-governance/tasks.md`.

## Spec Status

- Requirements: implemented for B5 v1 read-only report.
- Design: implemented JSON source-of-truth plus Markdown sibling.
- Tasks / checklist: implementation, tests, and handoff complete for subagent owner scope; main agent should handle status backfill/review/PR checklist.

## Next Notes

- Focused review should check report-dir normalization, source-of-truth JSON shape, and no DB initialization path.
- If future audit exposes per-path arrays for missing `status` / `compiled_to_wiki`, update spec before adding any dry-run proposal logic.
