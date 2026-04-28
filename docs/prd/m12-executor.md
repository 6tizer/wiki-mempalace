# PRD: M12 Executor

## Goal

Turn existing M12 strategy suggestions into an auditable execution lane without
weakening the current read-only default.

## Scope

- Add an executor dry-run plan derived from `StrategyReport`.
- Keep `wiki-cli suggest` default behavior read-only and unchanged.
- Preserve JSON as the machine-readable source of truth.
- Defer real writes to a later guarded apply step with allowlist and explicit
  apply flag.

## Non-Goals

- No automatic execution in the dry-run planner PR.
- No shell execution of `suggested_command`.
- No deletion, discard, force promote, cleanup, or disputed semantic replacement.
- No production apply without a prior dry-run artifact.

## Success Criteria

- `wiki-cli suggest --executor-plan` emits an action plan without DB, outbox, or
  Vault projection mutation.
- `--json --executor-plan` emits both the source strategy report and derived
  executor plan.
- `--report-dir --executor-plan` writes timestamped JSON/Markdown siblings for
  the report and the plan.
- Future guarded apply can consume typed plan fields instead of parsing shell
  commands.

## Status

- **Dry-run planner complete in PR #81** — `suggest --executor-plan` emits typed
  dry-run plans and report siblings.
- **Guarded apply complete in PR #82** — consume plan JSON with explicit
  `--apply` + allowlist.
- M12 executor roadmap item is complete.
