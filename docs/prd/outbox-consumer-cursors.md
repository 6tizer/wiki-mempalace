# PRD: Outbox Consumer Cursors

## Status

Implemented in PR #62/#63.

## Goal

把 outbox 导出从 caller-managed `last_id` 推进到 consumer-scoped cursor，降低重复派发和错误 ack 风险，同时保留现有手动导出兼容路径。

## Scope

- Phase 1: expose a storage API that exports NDJSON from a consumer's own cursor.
- Phase 1: keep existing `export_outbox_ndjson_from_id(last_id)` and CLI behavior unchanged.
- Phase 2: cut `consume-to-mempalace` and export CLI over to consumer cursor defaults.

## Out of Scope

- 不改变 `WikiEvent` payload schema。
- 不改变 Mempalace sink dispatch semantics。
- 不删除 legacy `processed_at` / `consumer_tag` columns；它们仍是观测字段。

## Success Criteria

- A consumer with no progress exports from `0`.
- After a consumer ack, that consumer exports only later events.
- A different consumer remains independent and still exports from its own cursor.
- Existing manual `last_id` floor keeps working as a legacy/manual override.
- `consume-to-mempalace` cannot rewind a consumer cursor by passing a lower `--last-id`.
