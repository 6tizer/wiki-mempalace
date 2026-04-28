# Requirements: M12 Executor Guarded Apply

## Functional Requirements

- **R1 plan input**: Apply must consume a JSON `StrategyExecutionPlan` produced by
  `wiki-cli suggest --executor-plan`.
- **R2 dry-run-first**: Without `--apply`, the command validates the plan and
  prints an execution preview without writing DB, outbox, Vault, or Mempalace.
- **R3 explicit apply**: Writes require `--apply`.
- **R4 allowlist**: Writes also require at least one explicit `--allow` value.
  This PR only supports `--allow fix-auto-safe`.
- **R5 typed dispatch**: Apply must use typed `action_kind` and evidence fields,
  not shell-execute `command_preview`.
- **R6 current-state validation**: Before applying, the command must confirm the
  planned auto fix still exists in the current lint/gap scan.
- **R7 audit report**: The command must emit text/JSON execution reports and
  optionally write JSON/Markdown report siblings.

## Acceptance Criteria

- [x] Preflight mode does not mutate wiki state.
- [x] `--apply` without `--allow` fails.
- [x] `--apply --allow fix-auto-safe` applies only matching auto fixes.
- [x] Unknown, stale, non-allowlisted, or human/agent-review actions are blocked
  or skipped.
- [x] Execution report records applied / blocked / skipped counts.

## Constraints

- No deletion, discard, force promote, cleanup, supersede, crystallize, or
  agent-review execution in this PR.
- `command_preview` remains audit display only.
- Production apply still requires operator-chosen plan and flags.
