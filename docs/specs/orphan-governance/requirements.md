# Requirements: Orphan Governance

## Goal

- Define safe orphan source cleanup after B1 produces a fresh audit report.

## Plain-Language Summary

- What this module does: turns fresh orphan evidence into a cleanup plan.
- Who it talks to: B1 audit output and vault Markdown.
- What user decision it implements: do not mutate orphan links or
  `compiled_to_wiki` until current evidence is reviewed.

## Functional Requirements

- Consume B1 fresh orphan categories.
- Separate at least:
  - title/link matched but not linked
  - compiled true but no page found
  - compiled false
  - missing `compiled_to_wiki`
  - unsupported or malformed source
- Produce a cleanup plan before implementation.
- Require dry-run before any vault mutation.
- Require user approval before any link rewrite or `compiled_to_wiki` change.

## Non-Goals

- Do not run before B1 report exists.
- Do not block B1-B4 completion.
- Do not auto-rerun LLM for all orphan sources.

## Inputs / Outputs

- Input: B1 audit report.
- Output: orphan cleanup requirements/design/tasks update or follow-up PRD if
  scope grows.

## Acceptance Criteria

- B5 starts only after B1 report.
- Cleanup categories and mutation policy are explicit.
- User approves before apply mode exists.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is rejected or ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: all cleanup policy and apply behavior.
- Agent can automate: report reading and proposed plan.
