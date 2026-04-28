# Requirements: M12 Executor Dry-Run Planner

## Functional Requirements

- **R1 plan model**: Add a serializable `StrategyExecutionPlan` derived from
  `StrategyReport`.
- **R2 typed actions**: Each action must include `suggestion_id`, `code`,
  `subject`, `execution_policy`, `action_kind`, `dry_run_status`,
  `command_preview`, and `reason`.
- **R3 no writes**: Planner must not mutate DB, outbox, Vault projection, or
  Mempalace.
- **R4 opt-in CLI**: `wiki-cli suggest` keeps existing output unless
  `--executor-plan` is supplied.
- **R5 JSON contract**: `--json --executor-plan` emits an envelope with the
  source strategy report and derived executor plan.
- **R6 report artifacts**: `--report-dir --executor-plan` writes plan JSON and
  Markdown siblings beside the existing suggestion report artifacts.

## Acceptance Criteria

- [x] Empty DB plan has stable JSON shape and zero actions.
- [x] Allowlisted auto-safe suggestions map to `would_apply` dry-run actions.
- [x] Agent-review and human-required suggestions map to `blocked`.
- [x] CLI text, JSON, and report-dir paths are tested.
- [x] Default `wiki-cli suggest` behavior stays compatible.

## Constraints

- `command_preview` is for audit display only. The next guarded apply PR must
  dispatch typed actions and allowlist entries, not shell strings.
- `dry_run_status=would_apply` means eligible for future guarded apply; it does
  not execute in this PR.
