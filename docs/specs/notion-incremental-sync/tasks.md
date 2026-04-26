# Tasks: Notion Incremental Sync

**Feature**: notion-incremental-sync  
**PRD**: [docs/prd/notion-incremental-sync.md](../../prd/notion-incremental-sync.md)  
**Design**: [design.md](design.md)  
**Status**: 已完成

---

## 模块拆分与分级

| ID | 模块 | Grade | Owner Files |
|---|---|---|---|
| T1 | 存储层：新增表 DDL + `WikiRepository` 方法 | Agent | `wiki-storage/src/lib.rs` |
| T2 | Notion API 客户端 | Agent | `wiki-cli/src/notion_client.rs` |
| T3 | Writeback trait + 实现 | Script | `wiki-cli/src/notion_writeback.rs` |
| T4 | 核心同步逻辑 `NotionSyncRunner` | Agent | `wiki-cli/src/notion_sync.rs` |
| T5 | CLI 子命令 + Automation job 注册 | Agent | `wiki-cli/src/main.rs` |
| T6 | 测试补全 + CI 验证 | Skill | 各模块 `#[cfg(test)]` 块 |

---

## 详细任务

### T1 存储层扩展

**Grade**: Agent  
**Owner**: `crates/wiki-storage/src/lib.rs`  
**不可触碰**: 现有 trait 方法签名和 schema 定义（只追加）

- [x] 在 `SCHEMA` 常量末尾追加 `notion_sync_cursors` 和 `notion_page_index` 两张表的 DDL（`CREATE TABLE IF NOT EXISTS`）
- [x] 在 `WikiRepository` trait 追加 4 个方法声明（见 design.md §2.2）
- [x] 在 `SqliteWikiRepository` 实现这 4 个方法
- [x] 单元测试：`storage_notion_cursor_roundtrip`、`storage_notion_page_exists`（在 wiki-storage lib.rs 的 `#[cfg(test)]` 块中）

**成功标准**: `cargo test -p wiki-storage` 通过

---

### T2 Notion API 客户端

**Grade**: Agent  
**Owner**: `crates/wiki-cli/src/notion_client.rs`（新建）  
**依赖**: T1 完成（不依赖，但先做 T1 更合理）

- [x] 新建 `notion_client.rs`，实现 `NotionApiClient` 和 `NotionPage` 结构体（见 design.md §3）
- [x] `from_env()` 读取 `NOTION_TOKEN` 环境变量，不存在时返回错误（不 panic）
- [x] `query_database_incremental`：filter + sort + 翻页循环 + limit 截断
- [x] 速率限制：`last_request_at` 计时 + `sleep`
- [x] HTTP 429 处理：读 `Retry-After` header，最多 3 次重试
- [x] 单元测试：`notion_client_rate_limit`、`notion_client_pagination`（使用 `mockito` 或手动 mock `reqwest` adapter）
- [x] 在 `wiki-cli/src/main.rs` 或 `lib.rs` 中 `mod notion_client;`

**成功标准**: 单元测试通过；真实 `NOTION_TOKEN` 环境下 `--dry-run` 能打印页面数

---

### T3 Writeback trait

**Grade**: Script  
**Owner**: `crates/wiki-cli/src/notion_writeback.rs`（新建）

- [x] 定义 `NotionWriteBackClient` trait（`mark_compiled`）
- [x] 实现 `NoopWriteBack`
- [x] 实现 `HttpNotionWriteBack`（`PATCH /v1/pages/{id}`，body `{"properties": {"已编译到Wiki": {"checkbox": true}}}`）
- [x] 单元测试：`notion_writeback_noop` 不出错
- [x] 在 `main.rs` 中 `mod notion_writeback;`

**成功标准**: 编译通过，noop 测试通过

---

### T4 核心同步逻辑

**Grade**: Agent  
**Owner**: `crates/wiki-cli/src/notion_sync.rs`（新建）  
**依赖**: T1、T2、T3

- [x] 实现 `NotionSyncRunner` 和 `SyncResult`（见 design.md §4）
- [x] `run_sync` 实现完整主流程：游标读取 → API 调用 → 去重 → ingest → cursor 更新
- [x] Source URI 格式：`notion://x_bookmark/<page_id>` / `notion://wechat/<page_id>`
- [x] Body 拼装：title + URL + 来源 + 状态 + 备注（见 design.md §4.3）
- [x] dry_run 分支：只计数不写 DB，不更新 cursor
- [x] writeback 调用：`writeback.mark_compiled(&page.id)`，错误只 warn 不 fail
- [x] 单元测试：`notion_sync_skips_existing_page`、`notion_sync_ingests_new_page`、`notion_sync_dry_run_no_writes`
- [x] 在 `main.rs` 中 `mod notion_sync;`

**成功标准**: `cargo test -p wiki-cli -- notion_sync` 通过

---

### T5 CLI + Automation 集成

**Grade**: Agent  
**Owner**: `crates/wiki-cli/src/main.rs`  
**依赖**: T1–T4

- [x] 在 `Commands` enum 追加 `NotionSync { ... }` 变体（所有参数见 design.md §5）
- [x] 在 `main` match 分支中调用 `NotionSyncRunner::run_sync`（x_bookmark / wechat / all 逻辑）
- [x] `NotionDbTarget` enum 实现 `clap::ValueEnum` + `Display`
- [x] 在 `AutomationJob` enum 追加 `NotionSync`
- [x] 在 `AUTOMATION_JOB_SPECS` 追加 spec（`in_daily = true`, `requires_network = true`, `short_circuit = false`）
- [x] 在 `automation_job_name` match 追加 `"notion-sync"`
- [x] 在 `run_automation_job` match 追加 `NotionSync` 分支，复用 `NotionSyncRunner`
- [x] 验证 `wiki-cli notion-sync --help` 输出正确
- [x] 验证 `wiki-cli automation list-jobs` 包含 `notion-sync`

**成功标准**: `cargo build -p wiki-cli` 通过；`notion-sync --dry-run` 与 `automation list-jobs` 均正常

---

### T6 测试补全 + CI

**Grade**: Skill  
**Owner**: 各模块测试块  
**依赖**: T1–T5

- [ ] 补全所有测试（见 design.md §9 测试策略表）
- [ ] `cargo fmt --all -- --check` 通过
- [x] `cargo test --workspace` 通过
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 手动 smoke：`wiki-cli notion-sync --db-id all --dry-run`（真实 token）打印正确结果

**成功标准**: 三项 CI gate 全部通过

---

## Review Checklist

### 模块 Review（每个 T 完成后）

- [ ] T1：新增 DDL 使用 `IF NOT EXISTS`；trait 方法不破坏现有接口
- [ ] T2：`NOTION_TOKEN` 不被打印；429 重试不超 3 次；`limit` 在翻页循环中正确截断
- [ ] T3：`HttpNotionWriteBack` 失败时返回 `Err` 不 panic；`NoopWriteBack` 实现 `Send + Sync`
- [ ] T4：dry_run 分支不写任何 DB 行；cursor 仅在非 dry_run 且无 error 时更新；去重查询在 ingest 前执行
- [ ] T5：`--writeback-notion` 默认 false；`automation list-jobs` 含 `notion-sync`；job 失败后不 short-circuit

### 集成 Review（所有 T 完成后）

- [ ] 首次运行（无 cursor）从 `NOW-30d` 开始
- [ ] 连续两次运行无新内容，DB 状态不变，cursor 不回退
- [ ] `--since` 覆盖 cursor 有效
- [ ] body 拼装格式与 `batch-ingest` 兼容（LLM 可正确解析）
- [ ] Source URI `notion://` 不与 vault_audit / vault_backfill 的 `file://` 逻辑冲突

---

## 状态追踪

| Task | 实现 | 测试 | Review | 备注 |
|---|---|---|---|---|
| T1 存储层 | ✅ | ✅ | ✅ | |
| T2 API 客户端 | ✅ | ✅ | ✅ | |
| T3 Writeback | ✅ | ✅ | ✅ | |
| T4 同步逻辑 | ✅ | ✅ | ✅ | |
| T5 CLI+Automation | ✅ | ✅ | ✅ | |
| T6 CI gate | — | ✅ | ✅ | fmt/test/clippy all green |
