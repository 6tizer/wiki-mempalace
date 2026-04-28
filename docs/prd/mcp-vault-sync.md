# PRD: MCP Vault Sync

## Status

Implemented in PR #61.

## Goal

让 MCP 写工具在 server 启用 `--wiki-dir --sync-wiki` 时自动刷新 Vault projection，避免 DB 已写入但 Obsidian Vault 仍停留旧状态。

## Scope

- MCP 写工具完成 DB/outbox persistence 后调用 `write_projection`。
- 只有 `--sync-wiki` 启用时才把 `wiki_dir` 传入 MCP runtime。
- projection 失败返回 typed MCP `storage_error`。
- 保持 MCP tool 成功响应 payload 不变。

## Out of Scope

- 不改变 `write_projection` ownership；仍只维护 `pages/`、`index.md`、`log.md`。
- 不消费 outbox 到 Mempalace；那仍由 `consume-to-mempalace` 负责。
- 不实现 multi-process writer lock/lease。

## Success Criteria

- MCP write with wiki sync writes `index.md` / `log.md` / managed pages.
- MCP without wiki sync does not write Vault projection.
- Projection failure is visible as typed `storage_error`.
- Focused MCP tests、workspace tests、fmt、clippy 通过。
