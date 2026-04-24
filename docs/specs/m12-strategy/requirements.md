# Requirements: M12 Strategy Suggestions

## Goal

- Produce explainable, read-only next-action suggestions from lint, gap, query, lifecycle, and metrics signals.

## Functional Requirements

- Add suggestion data model.
- Add at least 2 suggestion types.
- Add CLI command.
- Output reason and suggested command.
- Default behavior must not mutate wiki state.

## Non-Goals

- No automatic supersede/crystallize execution.
- No LLM judge in first version.

## Inputs / Outputs

- Input: store, lint/gap findings, query/outbox history, metrics report.
- Output: strategy suggestions in text; optional report page/file.

## Acceptance Criteria

- At least 2 suggestion types tested.
- `--viewer-scope` enforced.
- Suggestions include reason and command.

## User / Agent Gates

- User approval needed: first suggestion rule set.
- Agent can automate: implementation/tests/docs/review.
