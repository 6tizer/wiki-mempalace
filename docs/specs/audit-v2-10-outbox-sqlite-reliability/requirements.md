# Requirements: Audit v2 PR 10 Outbox + SQLite Reliability

## Scope

按 roadmap PR 10 修复 outbox ack 与 SQLite 写入可靠性：

- `mark_outbox_processed` 返回值按当前 `consumer_tag` 自己的 cursor 计算。
- manual ack 的目标不能超过当前 outbox head，避免 cursor 跳到未来。
- legacy `wiki_outbox.processed_at` / `consumer_tag` 不能影响新 consumer 的 ack 计数和 cursor。
- `SqliteRepository::open` 配置 SQLite `busy_timeout`。
- storage 层多步骤写事务统一走一个 `BEGIN IMMEDIATE` wrapper，减少 commit/rollback 分叉。
- 增加 locked/busy 回归测试。

## Acceptance

- 第二个 consumer ack 已被第一个 consumer 标记 `processed_at` 的事件时，返回本 consumer 的新增 ack 数。
- manual ack 大于当前 head 时，只推进到当前 head，后续新事件仍可导出。
- 同一个 consumer 重复 ack 不重复计数，不回退 cursor。
- legacy `processed_at` 仍可用于旧观测字段，不作为多 consumer 进度真源。
- 短暂 SQLite write lock 下写入会等待并成功。
- `cargo test -p wiki-storage` 覆盖 ack / busy / transaction 相关路径。

## Non-goals

- 不移除 `wiki_outbox.processed_at` / `consumer_tag` legacy columns。
- 不改变 outbox event payload schema。
- 不改 `consume-to-mempalace` live palace 写入行为。
- 不运行生产 `/Users/mac-mini/Documents/wiki` 写操作。
