# Tasks: Governance Automation Docs

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Spec setup | Main | `docs/specs/governance-automation-docs/*` | Complete |
| Automation registry | Main | `crates/wiki-cli/src/main.rs` | Complete |
| Automation tests | Main | `crates/wiki-cli/tests/automation_run_daily.rs` | Complete |
| Docs sync | Main | `docs/`, `llm-config.example.toml` | Complete |
| Handoff + lessons | Main | `docs/handovers/governance-automation-docs/`, `docs/LESSONS.md` | Complete |
| Gates | Main | fmt, tests, clippy, deny | Complete |

## Verification Checklist

- [x] `cargo fmt --all -- --check`
- [x] `cargo test -p wiki-cli --bin wiki-cli automation_job_registry_lists_named_jobs_in_stable_order --quiet`
- [x] `cargo test -p wiki-cli --test automation_run_daily automation_run_lint_executes_only_target_job --quiet`
- [x] `cargo test -p wiki-cli --test automation_run_daily automation_run_governance_scan_writes_report --quiet`
- [x] `cargo test -p wiki-cli --test automation_run_daily automation_run_fixer_plan_writes_report --quiet`
- [x] `cargo test -p wiki-cli --test automation_run_daily automation_run_fixer_apply_writes_apply_report_without_existing_plan --quiet`
- [x] `cargo test -p wiki-cli --test automation_run_daily automation_run_synthesis_jobs_write_reports_without_candidates --quiet`
- [x] `git diff --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
- [x] `cargo deny --all-features check advisories bans licenses sources`
