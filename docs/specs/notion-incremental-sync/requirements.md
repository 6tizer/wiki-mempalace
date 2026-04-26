# Requirements: Notion Incremental Sync

**Feature**: notion-incremental-sync  
**PRD**: [docs/prd/notion-incremental-sync.md](../../prd/notion-incremental-sync.md)  
**Status**: 草稿

---

## 功能需求

### FR-01 增量游标

- 系统必须在 `wiki.db` 中维护每个 Notion DB 的同步游标（`last_synced_at: ISO8601 UTC`）。
- 每次成功同步完成后，游标必须更新为本次 API 查询开始时刻（不是结束时刻，避免边界遗漏）。
- 游标必须按 `db_id` 独立维护。

### FR-02 增量过滤

- API 查询必须使用 `filter.last_edited_time.on_or_after = cursor` 参数。
- 首次同步（无游标）默认从 `NOW - 30d` 开始；可通过 `--since <ISO8601>` 覆盖。
- 游标之后的页面按 `last_edited_time DESC` 排序拉取，直到 `has_more = false` 或达到 `--limit`。

### FR-03 本地去重

- 系统必须在 `wiki.db` 中维护 `notion_page_id → source_id` 映射（`notion_sync_state` 表的 `page_id` 列）。
- 对于已存在 `notion_page_id` 的页面，跳过 ingest，记录 `skipped` 计数。
- 跳过不报错，只在 `--verbose` 日志中打印。

### FR-04 Raw Ingest

- 新页面必须转换为 body 文本（格式见 design.md §4.3），调用 `LlmWikiEngine::ingest_raw_with_tags`。
- Source URI 格式：`notion://x_bookmark/<page_id>` 或 `notion://wechat/<page_id>`。
- tags 取自 Notion 页面的 `标签` multi_select 字段，归一化后写入。
- scope 使用 CLI `--viewer-scope` 参数传入的值（与其他子命令一致）。

### FR-05 速率限制

- 相邻两次 Notion API 请求之间必须至少间隔 350ms（可通过 `--request-delay-ms` 覆盖，最小 100ms）。
- 收到 HTTP 429 时，读取 `Retry-After` 响应头（秒数，默认 60），等待后重试，最多 3 次。
- 3 次重试后仍 429，终止本次同步，标记 automation run 为 failed，输出 rate limit 错误。

### FR-06 CLI 子命令

`wiki-cli notion-sync` 必须支持以下参数：

| 参数 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `--db-id <ID>` | `x_bookmark\|wechat\|all` | `all` | 指定同步哪个 DB |
| `--since <ISO8601>` | 可选 | 游标 or NOW-30d | 覆盖增量起始时间 |
| `--limit <N>` | 可选 usize | 无限制 | 最多拉取 N 条页面 |
| `--dry-run` | flag | false | 打印将拉取页数，不写 DB |
| `--request-delay-ms <N>` | u64 | 350 | API 请求间隔（最小 100） |
| `--writeback-notion` | flag | false | 同步后标记 Notion 页面（首版关闭） |
| `--verbose` | flag | false | 打印每条 page_id 处理结果 |

### FR-07 Automation Job

- `notion-sync` 必须注册到 `AUTOMATION_JOB_SPECS`，`run_in_daily_chain = true`，`job_name = "notion-sync"`。
- job 执行时使用与 CLI 相同的 ingest 路径；job 失败不中断 automation chain 的后续 job（`short_circuit = false`）。
- job 必须通过 `start_automation_run` / `mark_automation_run_succeeded` / `mark_automation_run_failed` 记录状态。

### FR-08 Writeback 接口（首版关闭）

- 必须定义 `NotionWriteBackClient` trait，方法签名：`fn mark_compiled(&self, page_id: &str) -> Result<(), WriteBackError>`。
- 必须提供 `NoopWriteBack`（默认实现，无操作）和 `HttpNotionWriteBack`（调 Notion API `PATCH /pages/{page_id}` 设置 `已编译到Wiki` checkbox）。
- `--writeback-notion` flag 为 false（默认）时，使用 `NoopWriteBack`。

### FR-09 Dry-run

- `--dry-run` 必须：查询 Notion API、打印将新增的页面数、打印将跳过的页面数，不写 DB、不更新游标。

---

## 非功能需求

### NFR-01 幂等性

连续多次运行（无新内容时），DB 状态不变，游标不回退。

### NFR-02 可观测性

- stdout：每次运行结束打印 `fetched=N new=N skipped=N errors=N duration=Xs` 摘要。
- `automation health` / `automation last-failures` 中可见 notion-sync 的运行状态。

### NFR-03 安全性

- NOTION_TOKEN 只从环境变量读取，不接受 CLI 参数，不打印到 stdout/stderr。

### NFR-04 向后兼容

- 不改变现有子命令的行为。
- 新增 `notion_sync_state` 表通过 `CREATE TABLE IF NOT EXISTS` 升级，不影响已有 wiki.db。

---

## 约束

- 不引入新的异步运行时（`wiki-cli` 是同步 CLI；HTTP 调用通过 `reqwest::blocking` 完成）。
- 不依赖 `wiki-migration-notion` crate（那是离线 zip 解析器；字段解析在新模块内重写针对 API JSON）。
- `NotionWriteBackClient` 和 `HttpNotionWriteBack` 必须在测试中可 mock（trait object）。
