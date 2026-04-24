# Requirements: M11 Dashboard

## Goal

- Generate a read-only operations dashboard/report from health, metrics, and outbox state.

## Functional Requirements

- Add dashboard command.
- Generate static HTML or Markdown file.
- Include automation health, failures, metrics summary, outbox backlog, consumer status.
- Work without palace DB.

## Non-Goals

- No persistent web server.
- No write actions from dashboard.
- No auth/multi-user UI.

## Inputs / Outputs

- Input: `wiki.db`, metrics output, health/doctor state.
- Output: dashboard file path.

## Acceptance Criteria

- Dashboard generated from temp DB.
- Contains health and metrics sections.
- Missing optional palace state does not fail.

## User / Agent Gates

- User approval needed: static HTML vs Markdown default.
- Agent can automate: render implementation, tests, docs.
