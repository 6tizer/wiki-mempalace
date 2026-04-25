# Outbox 持久化与消费语义

本文描述当前 `wiki.db` outbox、消费者进度和 ack 语义。事件内容见
[outbox-event-matrix.md](outbox-event-matrix.md)。

## 当前实现

- 事件类型：`wiki_core::WikiEvent`。
- 运行时缓冲：`wiki_kernel::LlmWikiEngine::outbox`。
- 持久化接口：`wiki_storage::WikiRepository::append_outbox()`。
- 事件表：`wiki_outbox(id, event_json, processed_at, consumer_tag)`。
- 消费者进度表：`wiki_outbox_consumer_progress(consumer_tag, acked_up_to_id, acked_at)`。
- 导出接口：`export_outbox_ndjson()` / `export_outbox_ndjson_from_id(last_id)`。
- CLI 命令：`export-outbox-ndjson`、`export-outbox-ndjson-from`、`ack-outbox`、`consume-to-mempalace`。

## 写入顺序

写入类 CLI 子命令完成后会自动：

1. `save_to_repo()` 保存 snapshot。
2. `flush_outbox_to_repo_with_policy()` 分批写入 outbox。
3. 若启用 `--sync-wiki`，同步 Markdown projection。

这个顺序保证主状态先落库，事件后追记，消费端通过 resolver 读取事件关联对象时不会读到空状态。

## 消费者进度

`wiki_outbox_consumer_progress` 是消费者进度真源。每个 `consumer_tag` 独立维护
`acked_up_to_id`，所以多个消费者可以各自 ack 同一批事件，不会互相阻塞或导致 replay 循环。

`wiki_outbox.processed_at` / `wiki_outbox.consumer_tag` 仍保留作 legacy 观测字段；它们不是多消费者语义的真源。

`mark_outbox_processed(up_to_id, consumer_tag)` 的语义：

- 若该 consumer 第一次 ack，新增 progress 行。
- 若 `up_to_id` 大于旧 progress，推进到新值。
- 若 `up_to_id` 小于或等于旧 progress，不回退。
- 返回值是该 consumer 自己本次新 ack 的事件数。

## mempalace 消费

`consume-to-mempalace --palace <db>` 使用 `LiveMempalaceSink` 写真实 `palace.db`。
live bank 由 `--viewer-scope` 派生：

- `private:cli` -> `cli`
- `private:batch-ingest` -> `batch-ingest`
- `shared:team1` -> `team1`

这保证默认 `private:cli` 事件不会被写入错误 bank 后又被 ack。

当前 live sink 默认写入 `PageWritten` 的高质量页面和 claim/supersede 事件。
`SourceIngested` 默认 no-op；source 仍写入 `wiki.db`，但不默认写入 palace drawer。
若 resolver 无法解析 required event，消费者必须报错停止，不能 ack 跳过。

## 幂等建议

消费端应以 `wiki_outbox.id` 或事件 payload 的稳定 key 做幂等，不应假设事件只投递一次。
外部消费者应使用独立 `consumer_tag`，不要复用 mempalace 的 tag。
