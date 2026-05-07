# PRD: wiki-agent Runtime Upgrade

## Goal

Upgrade `wiki-agent` from a local-first chat/TUI surface into a more capable
agent runtime while preserving the current source-of-truth and tool boundaries.

The upgrade must keep:

- `wiki.db` as the source of truth.
- Vault and `palace.db` as projections.
- Native Rust `ToolRegistry` as the default tool backend.
- MCP child-process access only as compatibility fallback.
- Raw chat transcript storage in `.wiki/wiki-agent.db`.

## Current State

The original wiki-agent batch is complete:

1. Shared AI core. Merged PR #113.
2. Native ToolRegistry plus MCP adapter. Merged PR #114.
3. CLI chat. Merged PR #115.
4. Local + web RAG answering. Merged PR #116.
5. Manager-worker sub agents. Merged PR #117.
6. Persistent memory + skill loop. Merged PR #118.
7. TUI. Merged PR #119.
8. Docs and E2E. Merged PR #120.

This follow-up batch starts from the existing `wiki-agent` runtime and upgrades
planning, evaluation, prompting, structured events, and terminal UX.

## Product Outcome

- Chat turns run through an explicit `Plan -> Act -> Observe -> Evaluate ->
  Answer` loop.
- Planner decisions are visible and task-aware instead of keyword-only.
- Evidence quality and tool failure are evaluated before final answers.
- CLI and TUI share structured agent events.
- TUI renders message blocks, tool calls, evidence, footer status, themes, and
  keyboard navigation suitable for daily use.

## PR Roadmap

| PR | Theme | Outcome |
| --- | --- | --- |
| 1 | Harness Core State Machine | Introduce the explicit ReAct-style runtime loop. |
| 2 | Task Planner v2 | Route by intent, evidence budget, tools, workers, risk, and retry policy. |
| 3 | Evaluator + Retry Policy | Add evidence/tool quality checks and bounded automatic retry. |
| 4 | Prompt Runtime Builder | Replace the short system prompt with scoped runtime prompt assembly. |
| 5 | Structured Agent Events | Unify plan/tool/retry/evidence/answer events for CLI and TUI. |
| 6 | TUI Message Blocks | Replace string concatenation with typed message blocks. |
| 7 | TUI Layout + Footer | Add adaptive layout and operational footer status. |
| 8 | Tool Call UX | Render spinner, duration, summary, failure, and collapse state. |
| 9 | Theme System | Add dark, light, and mono theme tokens. |
| 10 | Keyboard Navigation | Add session picker, tool collapse, plan panel, help overlay, and cancel. |
| 11 | Snapshot + Harness Tests | Add regression coverage and closeout documentation. |

## Execution Rules

- Each PR must finish its own scope before the next PR starts.
- Each PR must run `rust-codebase` self-review.
- If review, tests, CI, or GitHub review finds an issue, fix it before merge.
- Each PR merge must be followed by local `main` update plus Vera and GitNexus
  index refresh.
- Do not change historical facts in `docs/prd/wiki-agent.md`; this PRD is the
  source for the runtime-upgrade batch.

## Acceptance

- `cargo fmt --all -- --check` passes.
- `cargo test -p wiki-agent` passes.
- `cargo clippy -p wiki-agent --all-targets -- -D warnings` passes.
- `git diff --check` passes.
- `gitnexus detect_changes` shows expected affected runtime flows only.
- Final local acceptance uses Computer Use against the TUI or another
  interactive entrypoint.
