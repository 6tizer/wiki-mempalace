# Design: wiki-agent Runtime Upgrade

## Runtime

`harness.rs` owns the ReAct-style turn loop. It builds a `HarnessPlan` from the
planner, executes native actions through a delegate, observes evidence status,
evaluates the result, and then asks the LLM for the final answer.

The loop emits `ChatEvent` values for every visible state change:

- phase changes
- plan start
- tool start/finish/failure
- retry
- evidence ready
- evaluation finish
- answer ready

## Planner And Evaluator

`planner.rs` derives `TaskPlan` from prompt, web mode, viewer scope, and private
web permission. It prefers local evidence, enables web only when the task needs
freshness and policy allows it, and routes complex work to manager-worker roles.

`evaluator.rs` checks whether the current evidence pack is strong enough to
answer. It controls bounded retry and records explicit degraded reasons when
web evidence is blocked, unavailable, or not cross-verified.

## Prompt Runtime

`prompt.rs` builds the runtime system prompt from the selected profile, scope,
tool backend, web mode, memory rules, evidence rules, and output contract.
Native tools remain the normal path; MCP child access is only a compatibility
fallback.

## TUI

The TUI uses typed `MessageBlock` values instead of raw string concatenation.
Activity, plan, sessions, help, tool cards, evidence, and errors share the same
rendering pipeline.

Layout is adaptive:

- wide terminals show Conversation plus Activity/overlay.
- narrow terminals hide Activity and preserve input/help/footer.
- footer shows profile, session, web mode, memory, backend, tokens, tool count,
  and latency.

Keyboard navigation:

- `Ctrl-R`: session overlay.
- `Ctrl-P`: plan overlay.
- `Ctrl-T`: collapse or expand the last tool card.
- `?`: help overlay.
- `Esc`: close overlay, cancel thinking state, clear input, or quit.

## Regression Strategy

Unit tests provide stable snapshots for harness event order, retry bounds,
private web block status, message block rendering, and TUI layout behavior.
These tests keep the runtime behavior observable without requiring live LLM,
live web credentials, or an interactive terminal in CI.
