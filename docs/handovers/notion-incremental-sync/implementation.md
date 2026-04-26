# Handover: Notion Incremental Sync

**Feature**: notion-incremental-sync  
**Branch**: `cursor/notion-incremental-sync-3354`  
**Date**: 2026-04-26  
**Status**: 已闭环（PR 已合并 + 后续回填）

---

## 改动摘要

实现 Notion API 增量同步到 `wiki.db`，新增：

- `wiki-cli notion-sync` 手动触发子命令
- `AutomationJob::NotionSync` 自动注册到 daily chain
- `notion_sync_cursors` / `notion_page_index` 两张 SQLite 表
- 速率限制（350ms 间隔）+ HTTP 429 重试（最多 3 次）
- `NotionWriteBackClient` trait（默认 Noop，`--writeback-notion` 可启用 HTTP 实现）

---

## 修改文件

| 文件 | 变更类型 | 说明 |
|---|---|---|
| `crates/wiki-storage/src/lib.rs` | 追加 | DDL（2 张表）+ 4 个 `WikiRepository` trait 方法 + impl + 测试 |
| `crates/wiki-cli/src/notion_client.rs` | 新建 | Notion API HTTP 客户端，速率限制，429 重试，mockito 测试 |
| `crates/wiki-cli/src/notion_writeback.rs` | 新建 | WritBack trait + NoopWriteBack + HttpNotionWriteBack |
| `crates/wiki-cli/src/notion_sync.rs` | 新建 | NotionSyncRunner 核心逻辑，in-memory 测试 |
| `crates/wiki-cli/src/main.rs` | 追加 | NotionSync 命令、NotionDbTarget enum、AutomationJob::NotionSync、run_notion_sync_cmd/job |
| `crates/wiki-cli/Cargo.toml` | 追加 | `thiserror = "2"`, `[dev-dependencies] mockito = "1.7.2"` |
| `docs/prd/notion-incremental-sync.md` | 新建 | PRD |
| `docs/specs/notion-incremental-sync/` | 新建 | requirements.md / design.md / tasks.md |

---

## 暴露接口

### CLI

```bash
# 手动增量同步（两个 DB）
wiki-cli --db <path> notion-sync --db-id all

# dry-run 预览
wiki-cli --db <path> notion-sync --db-id all --dry-run

# 从指定时间同步
wiki-cli --db <path> notion-sync --since 2026-04-01T00:00:00Z

# automation daily chain 包含 notion-sync
wiki-cli --db <path> automation run-daily
wiki-cli --db <path> automation run --job notion-sync
```

### Notion DB IDs（常量在 main.rs）

```rust
const NOTION_DB_X_BOOKMARK: (&str, &str) = ("x_bookmark", "0d305291-2a5d-426c-8db8-903ed5bb7ddb");
const NOTION_DB_WECHAT: (&str, &str) = ("wechat", "16470107-4b68-810a-bc81-f90795cc29ad");
```

### WikiRepository 新方法

```rust
fn get_notion_sync_cursor(db_id: &str) -> Result<Option<OffsetDateTime>>;
fn upsert_notion_sync_cursor(db_id: &str, at: OffsetDateTime, pages_synced_increment: i64) -> Result<()>;
fn notion_page_exists(notion_page_id: &str) -> Result<bool>;
fn insert_notion_page_index(notion_page_id: &str, db_id: &str, source_id: &SourceId) -> Result<()>;
```

---

## 已知限制

- **内容更新**：已存在 `notion_page_id` 的 source 在后续编辑时只 skip，不更新内容。需独立 PRD 实现 update 语义。
- **automation job 速率限制**：`run_notion_sync_job` 已修复为优先读取 `NOTION_SYNC_DELAY_MS`，未设置时回退到 350ms。  
  该配置不再是 hardcoded（见后续回填 PR）。
- **Writeback 默认关闭**：`--writeback-notion` 实现完整但默认 false；首版与现有流程行为一致。

---

## 新增依赖

- `thiserror = "2"`（`wiki-cli` dependencies）
- `mockito = "1.7.2"`（`wiki-cli` dev-dependencies）

---

## 测试结果

- `cargo test --workspace`: 所有测试通过（新增 17 个测试）
- `cargo clippy --workspace --all-targets -- -D warnings`: 通过
- `cargo fmt --all -- --check`: 通过
- 手动 smoke test: `notion-sync --dry-run` 返回 X书签 782 + 微信文章 482 dry-run 页面

---

## 下一步建议

1. **立即可用**：合并后在生产环境运行 `wiki-cli notion-sync --db-id all`（不加 `--dry-run`）拉取 1264 条新 source，再运行 `batch-ingest` 做 LLM 编译。
2. **后续跟进**：Notion Archived Source Retirement（roadmap 已有条目）— 识别 `is_archived=true` 页面并退役本地 source。
3. **P3**：为 automation job 的 notion-sync 添加 `NOTION_SYNC_DELAY_MS` 环境变量覆盖。
4. **P3**：实现 source 内容更新语义（`notion_page_id` 已存在但 `last_edited_time` 更新的情况）。
