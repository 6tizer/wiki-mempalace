# Requirements: Audit Report Hardening

## Functional Requirements

- MCP server MUST cap each stdin JSON-RPC line at 10 MiB before JSON parsing.
- CLI and MCP LLM ingest MUST call `LlmIngestPlanV1::validate_bounds()` before dry-run output or engine mutation.
- `validate_bounds()` MUST reject unsupported version, oversized text fields, and excessive claims/concepts/entities/relationships/tags.
- ingest redaction MUST catch existing bearer/API/private-marker patterns plus common `sk-`, `ghp_`, `github_pat_`, `xoxb-`, email, phone, and credit-card-like samples.
- `flush_outbox_to_repo_with_policy()` MUST append each batch with one repository batch API call.
- SQLite repository MUST implement batch outbox append in a `BEGIN IMMEDIATE` transaction.
- `rust-mempalace` FTS query builder MUST quote sanitized tokens before `MATCH`.

## Non-Functional Requirements

- No change to `WikiEvent` JSON schema.
- No network calls in tests.
- Default behavior for normal small requests remains unchanged.

## Acceptance

- Unit tests cover MCP line cap, plan bounds, redaction expansion, FTS quoting.
- Existing workspace tests remain green.

