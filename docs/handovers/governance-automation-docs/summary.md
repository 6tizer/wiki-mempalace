# Handoff: Governance Automation Docs

Branch: `codex/governance-automation-docs`

PR: #107

## What Changed

- Added automation jobs:
  - `governance-scan`
  - `fixer-plan`
  - `fixer-apply`
  - `synthesis-discover`
  - `synthesis-run`
- Updated daily order to:
  `notion-sync -> batch-ingest -> governance-scan -> fixer-plan -> fixer-apply -> maintenance -> consume-to-mempalace -> vault-reports`.
- Added default report directories:
  - `reports/governance/`
  - `reports/fixer/`
  - `reports/synthesis/`
- Kept governance/Fixer/Synthesis batch operations CLI-only for MCP.
- Added Notion workflow capability matrix.

## Safety Notes

- `wiki.db` remains the only write source.
- `fixer-apply` and `synthesis-run` take the existing writer lease.
- `fixer-apply` writes tombstones through the PR4 restore model.
- No production `/Users/mac-mini/Documents/wiki` apply command was run.

## Verification

- `git diff --check` passed.
- `cargo fmt --all -- --check` passed.
- `cargo test --workspace` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo deny --all-features check advisories bans licenses sources` passed.
