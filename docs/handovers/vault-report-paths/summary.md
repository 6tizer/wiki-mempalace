# Module Handoff: Vault Report Paths

## Summary

- Fixed report output path handling so Agent-facing report commands follow
  `--wiki-dir` for relative paths.
- Kept no-`--wiki-dir` defaults compatible with historical `wiki/reports/...`
  locations.

## Files Changed

| File | Change | Reason |
| --- | --- | --- |
| `crates/wiki-cli/src/main.rs` | Added vault-relative path helpers and wired report outputs | Prevent cwd-local report artifacts |
| `crates/wiki-cli/tests/dashboard.rs` | Added default dashboard vault test | Cover `<wiki-dir>/reports/dashboard.html` |
| `crates/wiki-cli/tests/suggest.rs` | Added default and explicit relative report-dir vault tests | Cover M12 report directory behavior |
| `crates/wiki-cli/tests/metrics.rs` | Added relative metrics report vault test | Cover explicit report path behavior |
| `crates/wiki-cli/tests/automation_run_daily.rs` | Added relative health summary vault test | Cover automation summary behavior |
| `docs/specs/vault-report-paths/` | Added spec trio | Keep workflow state self-contained |
| `AGENTS.md`, `docs/specs/README.md` | Updated path rule and index | Make future Agent behavior repeatable |

## Public Interfaces

- `wiki-cli dashboard` default output:
  - with `--wiki-dir`: `<wiki-dir>/reports/dashboard.html`.
  - without `--wiki-dir`: `wiki/reports/dashboard.html`.
- `wiki-cli suggest --report-dir` without a value:
  - with `--wiki-dir`: `<wiki-dir>/reports/suggestions`.
  - without `--wiki-dir`: `wiki/reports/suggestions`.
- Relative values for `metrics --report`, `dashboard --output`,
  `suggest --report-dir`, and `automation health --summary-file` are
  vault-relative when `--wiki-dir` is set.

## Known Limits

- `--db`, `--llm-config`, and schema paths remain cwd-relative defaults by
  design; they are config/state inputs, not vault reports.
- No projection ownership rules changed.

## Dependencies

- Added: none.
- Changed: none.
- Alternatives considered: relying on Agent prompts to pass absolute paths was
  rejected because it does not prevent repeated cwd-local artifacts.

## Verification

- Commands:
  - `cargo test -p wiki-cli --test dashboard --quiet`
  - `cargo test -p wiki-cli --test suggest --quiet`
  - `cargo test -p wiki-cli --test metrics --quiet`
  - `cargo test -p wiki-cli --test automation_run_daily --quiet`
  - `cargo fmt --all -- --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- Result: all passed.

## Spec Status

- Requirements: complete.
- Design: complete.
- Tasks / checklist: complete except PR-opened state.

## Next Notes

- After merge, use normal commands with `--wiki-dir /Users/mac-mini/Documents/wiki`;
  Agents no longer need to hardcode absolute report output paths.
