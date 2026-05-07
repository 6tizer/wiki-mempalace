# Requirements: wiki-agent Runtime Upgrade

## Scope

This spec covers the 11-PR follow-up batch for `wiki-agent` runtime quality:
harness loop, planner, evaluator, prompt runtime, structured events, TUI blocks,
layout, tool cards, themes, keyboard navigation, and regression tests.

## Requirements

- Chat turns run through an explicit `Plan -> Act -> Observe -> Evaluate -> Answer`
  loop.
- Planner output includes intent, evidence budget, tools, workers, risk, and
  retry policy.
- Evidence evaluation detects weak evidence, web verification gaps, unsupported
  claims, and tool failures before answer finalization.
- Retry is bounded and degrades explicitly when evidence or tools remain weak.
- Prompt assembly includes runtime profile, viewer scope, tool backend, memory
  policy, evidence rules, private web policy, and output contract.
- Native Rust tools remain the default backend; MCP remains fallback only.
- CLI and TUI consume the same structured event model.
- TUI renders typed message blocks, tool calls, evidence, errors, adaptive
  layout, footer labels, theme tokens, and keyboard navigation.
- Regression tests cover harness loop, retry, private web block, tool events,
  TUI layout, and message rendering.

## Acceptance

- `cargo fmt --all -- --check`
- `cargo test -p wiki-agent`
- `cargo clippy -p wiki-agent --all-targets -- -D warnings`
- `git diff --check`
- GitNexus `detect_changes` reports expected affected runtime/test/docs scope.
- Final acceptance exercises the TUI or fallback interactive entrypoint.
