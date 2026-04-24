# Requirements: M10 Metrics Core

## Goal

- Provide unified read-only metrics over wiki state, automation state, outbox, lint/gap, and lifecycle.

## Functional Requirements

- Add metrics data model.
- Add aggregation over store / repo state.
- Add `wiki-cli metrics`.
- Include at least 5 metric groups.
- Support stable text output and either JSON output or markdown report.
- Respect `--viewer-scope`.

## Non-Goals

- No dashboard UI.
- No long-term time-series DB in first version.
- No mutation of wiki state.

## Inputs / Outputs

- Input: `wiki.db`, current snapshot, optional `--viewer-scope`, optional `--wiki-dir`.
- Output: text metrics report; optional JSON/report file.

## Acceptance Criteria

- Empty DB output stable.
- Populated DB counts source / claim / page correctly.
- Lint/gap severity distribution appears.
- Outbox head and consumer backlog appear.
- Output is reusable by dashboard and strategy modules.

## User / Agent Gates

- User approval needed: final metric groups and output shape.
- Agent can automate: implementation, tests, docs, review.
