# Requirements: Reliability Test Matrix

## Functional Requirements

- Tests MUST include DB transaction failure injection for batch writes.
- Tests MUST include a large local smoke case.
- Tests MUST include MCP malformed JSON parse behavior.
- Tests MUST include LLM malformed JSON extraction behavior.
- Tests MUST avoid network calls and production paths.

## Non-Functional Requirements

- Tests should stay inside quick/local CI budget.
- No new dependency.
- No behavior change outside test-only helper refactors.

## Acceptance

- `cargo test -p wiki-storage reliability -- --nocapture`
- `cargo test -p wiki-cli reliability -- --nocapture`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
