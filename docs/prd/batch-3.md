# PRD: Batch 3 P2 Maturity

## Summary

- Goal: complete P2 maturity layer for metrics, dashboard, strategy suggestions, tag governance, and LongMemEval automation.
- User value: make wiki-mempalace easier to operate long term with measurable health, low-touch evaluation, and controlled next-action suggestions.
- Success criteria: M10-M12 are implemented, Schema T2 tags are consumed, LongMemEval scheduled lane produces artifacts, and roadmap state is updated.

## Scope

In:

- M10 metrics core.
- M11 read-only dashboard/report.
- M12 strategy suggestions.
- Schema T2 tag governance.
- LongMemEval scheduled non-blocking benchmark.

Out:

- Required PR CI LongMemEval gate.
- Always-on web server.
- Fully automatic high-risk supersede/crystallize execution.
- Schema T3 tag auto-extend / active-use analytics.

## Modules

| Module | Goal | Owner area | Status |
| --- | --- | --- | --- |
| J9 Metrics Core | Unified metrics model, CLI, reports | `wiki-core`, `wiki-kernel`, `wiki-cli` | Merged PR #12 |
| J10 Dashboard | Static read-only dashboard/report | `wiki-cli`, docs | Merged PR #14 |
| J11 Strategy Suggestions | Explainable suggestions, no auto execution | `wiki-core`, `wiki-kernel`, `wiki-cli` | Merged PR #16 |
| J12 Tag Governance | Tags model + schema policy consumption | `wiki-core`, `wiki-kernel`, `wiki-cli` | Merged PR #13 |
| J13 LongMemEval Auto Benchmark | Scheduled `rust-mempalace` local retrieval baseline artifacts | `.github/workflows`, `scripts`, docs | Merged PR #19 |

## Acceptance

- `metrics` reports at least 5 core metric groups.
- dashboard/report summarizes health, metrics, backlog, and consumer state.
- strategy/suggest emits at least 2 explainable suggestion types and is read-only by default.
- Claim / Source / LlmClaimDraft tags are backward-compatible and schema policy is consumed.
- LongMemEval scheduled workflow uploads markdown/json artifacts for the
  `rust-mempalace` local retrieval baseline and is not a required PR check.
- `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Risks

- Metrics scope creep.
- Dashboard becoming UI-heavy.
- Strategy suggestions causing noisy or unsafe recommendations.
- Tags breaking old snapshots.
- LongMemEval network/dataset drift causing noisy scheduled failures.
- Semantic fusion benchmarking pulling external embedding cost, keys, and rate
  limits into the J13 baseline. This is deferred to J14.

## Rollout

- Branch strategy: one `codex/<module>` branch per module unless specs explicitly combine modules.
- PR sequence: J9 and J12 first, then J10/J11/J13.
- Merge gate: spec approved, module review done, integration review done, CI green.

## Status

- [x] PRD approved
- [x] Specs created
- [x] Modules implemented
- [x] CI green
- [x] Merged
- [x] Roadmap updated for completed M10/M11/M12/J12/J13 status

Batch-3 P2 maturity is complete: M10, M11, M12, Schema T2, and J13 are merged.
J14 Semantic Fusion Benchmark remains a future follow-up and is not part of
Batch-3 acceptance.
