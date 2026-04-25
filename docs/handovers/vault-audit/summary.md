# Vault Audit Handoff

- Owner: Subagent A
- Scope: B1 read-only vault audit core module
- Branch: `codex/vault-backfill-palace-init`

## Files

- `crates/wiki-cli/src/vault_audit.rs`
- `crates/wiki-cli/tests/vault_audit.rs`

## Implemented

- `scan_vault(vault_path)` scans files without mutating vault content.
- YAML-like frontmatter parser supports `key: value`, inline lists, and simple block lists.
- Report covers totals, source/page stats, frontmatter coverage, invalid UTF-8 counts, stable ID coverage, duplicate identity candidates, old `.wiki/orphan-audit.json`, fresh orphan candidate categories, and backfill readiness.
- Duplicate readiness uses the full duplicate path set; JSON report duplicate paths remain sampled.
- `write_json_and_markdown(report, report_dir)` validates that `report_dir` is under `<vault>/reports`.
- `write_json_and_markdown_in_vault_reports(report)` writes to `<vault>/reports`.

## Boundaries

- Main-agent integration added the `vault-audit` CLI command.
- Did not add dependencies.
- Did not write stable IDs, DB records, or orphan cleanup changes.

## Verification

- `cargo test -p wiki-cli --test vault_audit`
- `cargo fmt --all -- --check`
- `git diff --check`

## Review Follow-up

- RA P1 duplicate readiness fixed with full duplicate path set.
- RA P2 report output is constrained to `<vault>/reports`.
- RA P2 invalid UTF-8 source/page files are counted instead of skipped.
