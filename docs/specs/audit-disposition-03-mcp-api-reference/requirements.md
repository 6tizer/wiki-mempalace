# Requirements: Audit Disposition PR3 MCP API Reference

## Goal

Refresh the standalone MCP API reference so it matches current `tools_list()`,
runtime scope rules, bank derivation, result shapes, side effects, and typed
errors.

## Plain-Language Summary

- What this module does: make MCP docs safe to use as current client reference.
- Who it talks to: MCP clients, future agents, docs sync tests.
- What user decision it implements: I-8 “待修复”.

## Functional Requirements

- `docs/mcp-api-reference.md` must list every tool currently returned by `tools_list()`.
- Each tool row must include required args, optional args, writes, return shape, and notes.
- Reference must document limit clamping, 10 MiB line cap, scope capability, typed errors, and projection side effects.
- Reference must state mempalace bank is derived from server viewer scope.
- Reference must not list mempalace `bank_id` as a client argument.
- Tests must fail if a new MCP tool is added without doc coverage.

## Non-Goals

- Do not change MCP tool schemas except docs sync tests.
- Do not change runtime behavior.
- Do not document standalone `rust-mempalace mcp` as if it were the unified wiki server.

## Acceptance Criteria

- MCP tool doc sync test passes.
- Mempalace docs test confirms required/optional client arg columns do not expose `bank_id`.
- API reference includes all typed error kinds.
- Full workspace gate passes.

## Checklist

- [x] Behavior matches PRD scope
- [x] Inputs and outputs are explicit
- [x] Out-of-scope behavior is ignored safely
- [x] Error cases are covered

## User / Agent Gates

- User approval needed: none; 2026-04-30 disposition decision already approved.
- Agent can automate: docs edit, tests, review, PR, CI, merge.
