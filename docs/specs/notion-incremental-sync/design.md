# Design: Notion Incremental Sync

**Feature**: notion-incremental-sync  
**PRD**: [docs/prd/notion-incremental-sync.md](../../prd/notion-incremental-sync.md)  
**Status**: 草稿

---

## 1. 架构总览

```
wiki-cli notion-sync (新子命令)
    │
    ├─► NotionSyncRunner (新结构体, wiki-cli 内部模块 notion_sync.rs)
    │       │
    │       ├─► NotionApiClient (notion_client.rs)
    │       │       HTTP reqwest::blocking
    │       │       NOTION_TOKEN 从 env 读取
    │       │       速率限制 + 429 重试
    │       │
    │       ├─► WikiRepository (已有 wiki-storage)
    │       │       读写 notion_sync_state 表（新增）
    │       │       page_id → source_id 去重查询
    │       │
    │       ├─► LlmWikiEngine (已有 wiki-kernel)
    │       │       ingest_raw_with_tags → RawArtifact → wiki.db
    │       │       save_to_repo_and_flush_outbox_with_policy()
    │       │
    │       └─► NotionWriteBackClient (trait, notion_writeback.rs)
    │               NoopWriteBack (默认)
    │               HttpNotionWriteBack (flag 启用)
    │
    └─► AutomationJob::NotionSync (注册到现有 automation 体系)
```

**不新增 crate**：所有新代码在 `wiki-cli/src/` 下新增模块文件；不改变已有 crate 的公共接口。  
**不依赖 wiki-migration-notion**：字段解析针对 Notion API JSON 在本模块内独立实现，结构简单。

---

## 2. 存储设计

### 2.1 新增表：`notion_sync_state`

在 `wiki-storage/src/lib.rs` 的 `SCHEMA` 常量中追加：

```sql
CREATE TABLE IF NOT EXISTS notion_sync_cursors (
  db_id         TEXT PRIMARY KEY,   -- "x_bookmark" | "wechat"
  last_synced_at TEXT NOT NULL,     -- ISO8601 UTC，上次成功同步的开始时刻
  pages_synced  INTEGER NOT NULL DEFAULT 0  -- 累计新增条数（仅供参考）
);

CREATE TABLE IF NOT EXISTS notion_page_index (
  notion_page_id TEXT PRIMARY KEY,  -- Notion page UUID (hyphenated)
  db_id          TEXT NOT NULL,     -- "x_bookmark" | "wechat"
  source_id      TEXT NOT NULL,     -- wiki SourceId (UUID)
  synced_at      TEXT NOT NULL      -- ISO8601 UTC
);
```

### 2.2 WikiRepository 新增方法

在 `wiki-storage` 的 `WikiRepository` trait 和 `SqliteWikiRepository` 实现中添加：

```rust
// 读游标
fn get_notion_sync_cursor(&self, db_id: &str) -> Result<Option<OffsetDateTime>, StorageError>;
// 更新游标（事务内调用）
fn upsert_notion_sync_cursor(&self, db_id: &str, at: OffsetDateTime, pages_synced: i64) -> Result<(), StorageError>;
// 查 page_id 是否已存在
fn notion_page_exists(&self, notion_page_id: &str) -> Result<bool, StorageError>;
// 记录 page_id → source_id 映射
fn insert_notion_page_index(&self, notion_page_id: &str, db_id: &str, source_id: &SourceId) -> Result<(), StorageError>;
```

---

## 3. Notion API Client

**文件**: `crates/wiki-cli/src/notion_client.rs`

```rust
pub struct NotionApiClient {
    token: String,               // 来自 env::var("NOTION_TOKEN")
    request_delay_ms: u64,       // 默认 350
    max_retries: u32,            // 固定 3
    last_request_at: Instant,    // 速率限制计时
}

pub struct NotionPage {
    pub id: String,              // page UUID (hyphenated)
    pub last_edited_time: OffsetDateTime,
    pub title: String,
    pub url: Option<String>,     // 文章链接
    pub tags: Vec<String>,       // 标签 multi_select
    pub source: Option<String>,  // 来源 select
    pub note: Option<String>,    // 备注 rich_text
    pub status: Option<String>,  // 状态 select
}

impl NotionApiClient {
    pub fn from_env() -> Result<Self, SyncError>;
    pub fn query_database_incremental(
        &mut self,
        db_id: &str,
        since: Option<OffsetDateTime>,
        limit: Option<usize>,
    ) -> Result<Vec<NotionPage>, SyncError>;
}
```

**query_database_incremental 实现逻辑**:
1. 构造 `POST /v1/databases/{db_id}/query` body：
   ```json
   {
     "filter": {"timestamp": "last_edited_time", "last_edited_time": {"on_or_after": "<since>"}},
     "sorts": [{"timestamp": "last_edited_time", "direction": "descending"}],
     "page_size": 100
   }
   ```
2. 循环：`sleep(remaining_delay)` → 发请求 → 处理 429（`Retry-After` + 重试）→ 解析 results → 如 `has_more=true` 且未超 limit 则 cursor 翻页 → 累计结果。
3. 首次无 `since`：`since = now - 30d`。

**速率限制实现**:
```
每次 HTTP 请求前：
  elapsed = now - last_request_at
  if elapsed < request_delay_ms:
      sleep(request_delay_ms - elapsed)
  last_request_at = now
```

---

## 4. 核心同步逻辑

**文件**: `crates/wiki-cli/src/notion_sync.rs`

### 4.1 主流程

```rust
pub struct NotionSyncRunner { ... }

pub struct SyncResult {
    pub db_id: String,
    pub fetched: usize,
    pub new: usize,
    pub skipped: usize,
    pub errors: usize,
    pub duration_secs: f64,
}

impl NotionSyncRunner {
    pub fn run_sync(
        &mut self,
        db_id: &str,                  // "x_bookmark" | "wechat"
        notion_db_id: &str,           // Notion UUID
        since_override: Option<OffsetDateTime>,
        limit: Option<usize>,
        dry_run: bool,
        writeback: &dyn NotionWriteBackClient,
        verbose: bool,
    ) -> Result<SyncResult, SyncError>;
}
```

步骤：
1. `sync_started_at = now` （游标更新用）
2. 读 cursor：`repo.get_notion_sync_cursor(db_id)` → `since`
3. 若 `since_override` 不为 None，用 override 替换
4. 调 `client.query_database_incremental(notion_db_id, since, limit)` → pages
5. 对每条 page：
   - `if repo.notion_page_exists(&page.id)` → skipped++, continue
   - 如是 dry_run → 仅计数，不写 DB
   - 否则：构造 body（§4.3）→ `engine.ingest_raw_with_tags(uri, body, scope, "notion-sync", tags)` → source_id
   - `repo.insert_notion_page_index(&page.id, db_id, &source_id)`
   - new++
   - 如 `--writeback-notion`：`writeback.mark_compiled(&page.id)`（忽略错误，只打印 warn）
6. 如非 dry_run：`engine.save_to_repo_and_flush_outbox_with_policy(&repo)`
7. 如非 dry_run：`repo.upsert_notion_sync_cursor(db_id, sync_started_at, new)`
8. 返回 SyncResult

### 4.2 Source URI 格式

```
notion://x_bookmark/<notion_page_id_hyphenated>
notion://wechat/<notion_page_id_hyphenated>
```

与现有 `file://` URI 不冲突；vault_backfill 和 vault_audit 按前缀识别 URI 类型，`notion://` 开头会被归类为外部 URL（frontmatter `url: notion://...`），行为与 `https://` URI 的 source 一致。

### 4.3 Body 拼装格式

```
# {title}

URL: {url}
来源: {source}
状态: {status}
备注: {note}
```

空字段不输出对应行。此格式与 `batch-ingest` 期望的 source body 格式兼容，LLM 可从中提取结构。

---

## 5. CLI 子命令

**文件**: `crates/wiki-cli/src/main.rs`（在现有 `Commands` enum 追加）

```rust
/// Incrementally sync Notion databases into wiki.db.
NotionSync {
    /// Which DB to sync: x_bookmark | wechat | all
    #[arg(long, default_value = "all")]
    db_id: NotionDbTarget,

    /// Override incremental cursor (ISO8601 UTC)
    #[arg(long)]
    since: Option<String>,

    /// Max pages to fetch per DB
    #[arg(long)]
    limit: Option<usize>,

    /// Print what would be synced without writing
    #[arg(long)]
    dry_run: bool,

    /// Milliseconds between API requests (min 100)
    #[arg(long, default_value_t = 350)]
    request_delay_ms: u64,

    /// Write back to Notion after sync (marks 已编译到Wiki checkbox)
    #[arg(long)]
    writeback_notion: bool,

    /// Print per-page processing results
    #[arg(long)]
    verbose: bool,
}
```

```rust
enum NotionDbTarget {
    XBookmark,
    Wechat,
    All,
}
```

---

## 6. Writeback 接口

**文件**: `crates/wiki-cli/src/notion_writeback.rs`

```rust
pub trait NotionWriteBackClient: Send + Sync {
    fn mark_compiled(&self, page_id: &str) -> Result<(), WriteBackError>;
}

pub struct NoopWriteBack;
impl NotionWriteBackClient for NoopWriteBack {
    fn mark_compiled(&self, _page_id: &str) -> Result<(), WriteBackError> { Ok(()) }
}

pub struct HttpNotionWriteBack {
    token: String,
    client: reqwest::blocking::Client,
}
impl NotionWriteBackClient for HttpNotionWriteBack {
    fn mark_compiled(&self, page_id: &str) -> Result<(), WriteBackError> {
        // PATCH https://api.notion.com/v1/pages/{page_id}
        // body: {"properties": {"已编译到Wiki": {"checkbox": true}}}
        // 失败不 panic，返回 Err，调用方只 warn
    }
}
```

---

## 7. Automation Job 注册

在 `wiki-cli/src/main.rs` 的 `AutomationJob` enum 追加：

```rust
NotionSync,
```

在 `AUTOMATION_JOB_SPECS` 追加：

```rust
AutomationJobSpec {
    job: AutomationJob::NotionSync,
    run_in_daily_chain: true,
    short_circuit_on_failure: false,
},
```

在 `automation_job_name` match 追加：

```rust
AutomationJob::NotionSync => "notion-sync",
```

在 `run_automation_job` 中追加对应执行逻辑，复用 `NotionSyncRunner::run_sync`。

---

## 8. 模块文件清单

| 文件 | 说明 |
|---|---|
| `crates/wiki-cli/src/notion_client.rs` | Notion API HTTP 客户端（新建） |
| `crates/wiki-cli/src/notion_sync.rs` | 核心同步逻辑 `NotionSyncRunner`（新建） |
| `crates/wiki-cli/src/notion_writeback.rs` | Writeback trait + Noop + Http 实现（新建） |
| `crates/wiki-cli/src/main.rs` | 添加子命令 + automation job（追加） |
| `crates/wiki-storage/src/lib.rs` | 新增表 DDL + 4 个 trait 方法（追加） |
| `crates/wiki-cli/Cargo.toml` | 确认 `reqwest` with `blocking` feature（已有则无操作） |

---

## 9. 测试策略

| 测试 | 位置 | 说明 |
|---|---|---|
| `notion_client_rate_limit` | notion_client.rs | mock server 返回 429，验证 Retry-After 等待逻辑 |
| `notion_client_pagination` | notion_client.rs | mock server 返回 has_more=true，验证翻页累计 |
| `notion_sync_skips_existing_page` | notion_sync.rs | page_id 已在 notion_page_index，验证 skipped++ 且不写 DB |
| `notion_sync_ingests_new_page` | notion_sync.rs | page_id 不存在，验证 source 写入 wiki.db + cursor 更新 |
| `notion_sync_dry_run_no_writes` | notion_sync.rs | dry_run=true，验证 DB 不变、cursor 不变 |
| `notion_writeback_noop` | notion_writeback.rs | NoopWriteBack.mark_compiled 不出错 |
| `storage_notion_cursor_roundtrip` | wiki-storage | get/upsert cursor 往返 |
| `storage_notion_page_exists` | wiki-storage | insert + exists 往返 |

**不需要**真实 Notion API 调用的集成测试（使用 mock HTTP server 或直接注入 `Vec<NotionPage>`）。

---

## 10. 已知限制与延后事项

| 事项 | 说明 |
|---|---|
| 内容更新检测 | 首版只新增，已存在 page_id 跳过；内容变更场景（Notion 文章被编辑）待独立 PRD |
| `已编译到Wiki` 写回 | 接口完整，`--writeback-notion` 首版默认关闭 |
| Webhook/push 模式 | Out of scope；polling 模式足以支撑 12h 周期 |
| 三个 DB 以外的 Notion 库 | Out of scope |
