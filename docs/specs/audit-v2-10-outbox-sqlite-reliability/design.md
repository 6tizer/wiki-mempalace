# Design: Audit v2 PR 10 Outbox + SQLite Reliability

## Current Problem

Outbox consumer cursor 已经以 `wiki_outbox_consumer_progress` 为真源，但
`mark_outbox_processed_inner` 的返回计数仍过滤 `processed_at IS NULL`。

这会导致第二个 consumer ack 同一段事件时被旧 global processed marker 少算：

1. `mempalace` ack `1..=2`，写入 `processed_at`。
2. `archive` 第一次 ack `1..=3`。
3. 旧逻辑只返回 `1`，因为 `1..=2` 已有 `processed_at`。

这个返回值不应代表 global processed 数，而应代表该 consumer 的新增 ack 数。

SQLite 侧还有重复 `BEGIN IMMEDIATE` / `COMMIT` / `ROLLBACK` 代码，且 open 没有统一
`busy_timeout`，短暂 write lock 容易直接失败。

## Approach

### Per-consumer Ack Count

`mark_outbox_processed_inner`：

- 先读取当前 consumer 的 `previous_ack`，不存在则为 `0`。
- `up_to_id <= previous_ack` 返回 `0`。
- `effective_up_to_id = min(up_to_id, current_head_id)`，防止 manual ack 把 cursor 推到未来。
- `newly_acked` 使用 `id > previous_ack AND id <= effective_up_to_id` 计数，不再看 `processed_at`。
- 更新 `wiki_outbox_consumer_progress`。
- legacy `wiki_outbox.processed_at` 仍只填充尚为空的 rows，用于旧观测。

### SQLite Busy Timeout

`SqliteRepository::open` 设置 `busy_timeout = 5000ms`。

测试用第二 connection 持有短暂 `BEGIN IMMEDIATE` write lock，主 repo 写 outbox 需要等待锁释放并成功。

### Transaction Wrapper

新增 `SqliteRepository::immediate_transaction`，用于多步骤写事务：

- 统一 `BEGIN IMMEDIATE`。
- `Ok` 后 commit；commit 失败尝试 rollback 并返回 DB error。
- `Err` 后 rollback，并返回原 error。

多步骤写事务路径改为调用 wrapper，包括 snapshot/outbox、embedding、alias、notion index batch、outbox ack 等。
单条 insert/upsert 路径仍可由 rusqlite statement 自身事务处理。

## Compatibility

- `wiki_outbox_consumer_progress` schema 不变。
- legacy `processed_at` / `consumer_tag` column 保留。
- `OutboxStats.unprocessed_events` 仍按 legacy `processed_at IS NULL` 计算；consumer backlog 仍按 progress cursor 计算。
