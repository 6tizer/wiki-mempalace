# Requirements: Audit Disposition PR2 Doc Consistency

## Goal

Remove active documentation contradictions around Rust edition and Notion sync status.

## Plain-Language Summary

- What this module does: active docs should describe the current repo, not old audit-era state.
- Who it talks to: docs readers and future agents.
- What user decision it implements: L-4 “待修复”.

## Functional Requirements

- Active docs must not claim `rust-mempalace` uses a separate newer Rust edition.
- Active docs must state the workspace uses Rust edition 2021 and crates inherit it through `edition.workspace = true`.
- Active docs must not state or imply Notion incremental sync is unimplemented.
- Current architecture docs must keep the completed Notion sync / archived source retirement state visible.
- Archive files may keep historical wording, but archive index must warn that archived docs are stale context.

## Non-Goals

- Do not change Cargo edition or crate manifests.
- Do not rewrite archived history.
- Do not implement Notion sync or archived-source code changes.

## Acceptance Criteria

- Active-doc grep for the old edition wording returns no matches.
- Active docs contain no stale Notion sync “not implemented” status.
- `git diff --check` passes.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: none; 2026-04-30 disposition decision already approved.
- Agent can automate: docs edit, review, gates, PR, CI, merge.
