# PRD: Notion Incremental Sync

**批次**: notion-incremental-sync  
**状态**: 草稿 → 待用户确认  
**关联 roadmap**: `Notion Incremental Sync 💤 未开始`  
**背景**: architecture.md §0 "C. 未来: Notion API 增量同步"

---

## 1. 背景与动机

当前 X书签文章数据库和微信文章数据库（文章数据库）均通过一次性离线导出（`wiki-migration-notion`）迁移至 `wiki.db`。Notion 两个 DB 持续有新增/编辑内容，本地无法自动感知。

目标：通过 Notion API 实现两个 DB 的增量拉取，将新增条目按现有 `ingest` 路径写入 `wiki.db`，再经 outbox → vault / palace 派生层。

---

## 2. Database 信息

| 库 | Notion DB ID | 最近更新 |
|---|---|---|
| X书签文章数据库 | `0d305291-2a5d-426c-8db8-903ed5bb7ddb` | 2026-04-16 |
| 文章数据库（微信文章） | `16470107-4b68-810a-bc81-f90795cc29ad` | 2026-04-02 |

NOTION_TOKEN 已作为 secret 注入环境变量。

---

## 3. 产品范围

### 3.1 In Scope（本 PRD）

1. **增量拉取**：按 `last_edited_time` 游标，仅取上次同步之后新增/变更的页面。  
2. **本地去重**：`notion_page_id` 首次出现新建 source，已存在跳过（不重复插入）；更新检测留为 future follow-up。  
3. **Raw ingest**：提取 `标题 + 文章链接 + 标签 + 来源 + 备注` 拼成 body，调用 `LlmWikiEngine::ingest_raw`，写入 `wiki.db`。无 LLM，不生成 claim/page（与现有 `batch-ingest` 配合，后续由 LLM 编译）。  
4. **CLI 子命令**：`wiki-cli notion-sync`，支持手动触发。  
5. **Automation job 注册**：将 `notion-sync` 注册到 `AUTOMATION_JOB_SPECS`，接入 `run-daily` 链，12 小时周期。  
6. **速率限制处理**：固定 350ms 请求间隔；HTTP 429 读 `Retry-After`（默认 60s）后重试，最多 3 次。  
7. **写回接口（关闭）**：定义 `NotionWriteBackClient` trait + `NoopWriteBack`（默认）+ `HttpNotionWriteBack`（代码实现完整，`--writeback-notion` flag 启用，首版默认关闭）。  
8. **游标持久化**：`wiki.db` 新增 `notion_sync_state` 表（`db_id`, `last_synced_at`, `pages_synced`）。  

### 3.2 Out of Scope（本 PRD 不做）

- LLM 自动编译（由 `batch-ingest` job 负责，非本模块范围）。  
- 已有 source 的内容更新/覆盖（首版只做 "新增跳过已存在"，更新语义待独立 PRD）。  
- Notion archived 条目退役（已有独立 roadmap 条目 `Notion Archived Source Retirement`）。  
- `已编译到Wiki` checkbox 写回 Notion（在 `--writeback-notion` flag 后），首版关闭。  
- 三个 DB 以外的 Notion 数据库。  
- Webhook / push 模式。  

---

## 4. 用户决策确认

| 决策点 | 用户决定 |
|---|---|
| 触发方式 | 手动 CLI + automation job 定时（两者均支持） |
| 优先 DB | 两个 DB 同时支持（`--db-id x_bookmark\|wechat\|all`） |
| 内容入库方式 | raw ingest；LLM 编译由后续 `batch-ingest` 负责 |
| Notion token 管理 | 已作为 `NOTION_TOKEN` 环境变量注入 |
| Notion 写回 | 接口完整实现，首版默认关闭 |

---

## 5. 成功标准

1. `wiki-cli notion-sync --db-id all --dry-run` 打印将拉取的页面数，不写 DB。  
2. `wiki-cli notion-sync --db-id all` 成功写入新页面，游标更新，再次运行不重复插入相同 `notion_page_id`。  
3. `wiki-cli automation run --job notion-sync` 在 automation 体系中正常运行、记录 run state、heartbeat、health 可见。  
4. `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check` 全部通过。  

---

## 6. 依赖与风险

| 依赖 | 说明 |
|---|---|
| `NOTION_TOKEN` 环境变量 | 已注入 |
| Notion API v2022-06-28 | 稳定版本，无预期破坏性变更 |
| `reqwest` crate | 已在 workspace，需在 wiki-cli Cargo.toml 添加特性 |
| `wiki-kernel::LlmWikiEngine::ingest_raw` | 已有，无需改动 |

| 风险 | 缓解措施 |
|---|---|
| Notion API rate limit | 350ms 间隔 + 429 重试 + `--limit` 保护 |
| 首次同步量过大 | `--since` 覆盖游标，`--limit N` 截断 |
| 重复 source 插入 | `notion_sync_state` 表 page_id 去重，ingest 前查询 |

---

## 7. 状态

- [ ] PRD 用户确认  
- [ ] spec 三件套写完并确认  
- [x] 实现完成  
- [x] CI 通过  
- [ ] PR 合并  
- [ ] roadmap 状态回填  
