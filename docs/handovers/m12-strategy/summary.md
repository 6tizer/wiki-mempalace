# Module Handoff: M12 Strategy Suggestions

## Summary

- M12 added a read-only strategy suggestion lane through `wiki-cli suggest`.
- The command produces terminal text by default, supports canonical JSON through
  `--json`, and can write timestamped JSON/Markdown reports with `--report-dir`.
- JSON is the source of truth. Markdown is rendered from the same
  `StrategyReport`.
- First version diagnoses and dispatches only. It does not execute supersede,
  crystallize, fix, deletion, discard, force, or cleanup actions.

## Files Changed

| File | Change | Reason |
| --- | --- | --- |
| `crates/wiki-core/src/strategy.rs` | Added strategy report and suggestion model | Shared report contract |
| `crates/wiki-core/src/lib.rs` | Re-exported strategy types | CLI/kernel access |
| `crates/wiki-kernel/src/strategy.rs` | Added read-only strategy scan rules | Convert lint/gap/metrics signals into suggestions |
| `crates/wiki-kernel/src/lib.rs` | Exposed strategy scanner | CLI integration |
| `crates/wiki-cli/src/main.rs` | Added `suggest` command and report rendering | User-facing command |
| `crates/wiki-cli/tests/suggest.rs` | Added CLI/report tests | Regression coverage |
| `docs/specs/m12-strategy/*` | Updated requirements/design/tasks | Spec kept aligned with implementation |
| `docs/roadmap.md` and `docs/prd/batch-3.md` | Marked M12 merged | Planning state sync |
| `docs/LESSONS.md` | Recorded M12 merge lessons | Future planning input |

## Public Interfaces

- CLI: `wiki-cli suggest`
- CLI flags:
  - `--json`
  - `--report-dir [PATH]`, defaulting to `wiki/reports/suggestions` when the
    option is present without an explicit path
- Core model:
  - `StrategyReport`
  - `StrategySuggestion`
  - `StrategySeverity`
  - `StrategyExecutionPolicy`
- Kernel entry:
  - `run_strategy_scan(...)`

## Known Limits

- No execution path in M12 first version.
- No dashboard integration yet.
- No internal operator/executor yet.
- Query-history suggestions are conservative because `QueryServed` events do
  not carry explicit viewer scope and may contain raw query text.
- Schema T2 tags exist but are not required by first-version M12 rules.

## Dependencies

- Added: none.
- Changed: none.
- Alternatives considered:
  - Direct execution was deferred to avoid high-risk writes.
  - Dashboard consumption was deferred to keep M12 focused on report generation.
  - Query history rules were kept scope-safe instead of using unresolved events.

## Verification

- Commands:
  - `cargo test -p wiki-core strategy --quiet`
  - `cargo test -p wiki-kernel strategy --quiet`
  - `cargo test -p wiki-cli --test suggest --quiet`
  - `cargo test -p wiki-cli --test metrics --quiet`
  - `cargo test -p wiki-cli --test dashboard --quiet`
  - `cargo test -p wiki-core tag --quiet`
  - `cargo test -p wiki-kernel tag --quiet`
  - `cargo fmt --all -- --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- Result:
  - Local verification passed during PR #16.
  - GitHub `quick` CI passed.
  - PR #16 merged into `main` on 2026-04-25.
  - PR #17 recorded merge lessons and was also merged.

## Spec Status

- Requirements: complete for first-version M12.
- Design: complete for first-version M12.
- Tasks / checklist: complete, with follow-ups deferred.

## Next Notes

- Batch-3 next mainline is J13 LongMemEval Auto Benchmark.
- M12 follow-ups should be separate specs:
  - internal operator/executor that reads `*-m12-suggest.json`
  - dashboard latest suggestion report link
  - `QueryServed` scope/hash schema improvement
