# Dogfood 就绪清单

> 目标：**层级二**——1–2 天打磨后可作为个人知识库日常使用。
> 此清单聚焦"开始真实喂文章"前的最小必要工作，不是完整的 T1 打磨。

---

## 一、你需要配置（我替代不了）


| #   | 任务                                                                                                        | 产出位置                           | 耗时    | 状态  |
| --- | --------------------------------------------------------------------------------------------------------- | ------------------------------ | ----- | --- |
| U1  | 复制 `llm-config.example.toml` → `llm-config.toml`，填入真实 API key                                             | 代码仓库根目录（`.gitignore` 已排除）或数据目录 | 5 分钟  | ✅   |
| U2  | 选定**数据目录**（推荐 `~/wiki-mempalace/`，已选定 `/Users/mac-mini/Documents/wiki/`）                                  | 任意本地路径，**不要**放代码仓库里            | 5 分钟  | ✅   |
| U3  | 把 `DomainSchema.json` 拷贝到数据目录，**按自己节奏微调** `min_age_days` / `required_sections` / `stale_days`（现有默认值见附录 A） | `<数据目录>/DomainSchema.json`     | 30 分钟 | ✅   |
| U4  | 让 Obsidian / Logseq vault 指向 `<数据目录>/` — 已验证，4477 条 markdown 全部可浏览、内部边可点击、搜索正常                            | 编辑器内配置                         | 5 分钟  | ✅   |
| U5  | 决定是否开启 embeddings（`--vectors`）。已配 qwen3-embedding-8b，建议开                                                  | —                              | —     | ✅   |


### LLM 模型选择参考


| 工作                                                  | 调用场景                                 | 推荐模型                                                          |
| --------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------- |
| Chat completion（抽 claims / entities / summary JSON） | `ingest-llm` / MCP `wiki_llm_ingest` | **deepseek-chat**（便宜、中文好）或 `gpt-4o-mini` / `claude-3-5-haiku` |
| Embeddings（可选）                                      | `--vectors` 开启后                      | `openai text-embedding-3-small` 或不配                           |


---

## 二、我需要开发

按对"日常可用"的贡献从高到低：


| #   | 来源         | 任务                                                                                                                               | 价值                                                   | 估时     | 状态  |
| --- | ---------- | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ------ | --- |
| D1  | Bucket 4.1 | **projection 写 YAML frontmatter**（`id` / `status` / `entry_type` / `updated_at`），修改 `crates/wiki-kernel/src/wiki_writer.rs`，配套测试 | **决定性**——否则 Obsidian 里看不到 lifecycle 状态，T1 成果对用户完全不可见 | 半天     | ✅   |
| D2  | Bucket 1.2 | 在 `DomainSchema.json` 为 concept/entity 加反向 promotion 规则 `needs_update → approved`（否则页面 stale 后锁死）                                | 关键修复                                                 | 1 小时   | ✅   |
| D3  | 新增         | `ingest-llm` 不传 `--entry-type` 时默认 `concept`（或让 LLM 在 plan 里判断并写回 `LlmIngestPlanV1`）                                             | 每次 ingest 不用手动指定                                     | 1–2 小时 | ✅   |
| D4  | 新增         | `scripts/backup.sh`：cp SQLite db 到 `<数据目录>/backups/knowledge-YYYYMMDD-HHMM.db`                                                   | 防丢数据                                                 | 15 分钟  | ✅   |


**全部完成。**

---

## 三、暂不做（出问题再补）

以下来自 T1 打磨 5 个 Bucket，**不阻塞 dogfood**：

- Bucket 2.1：`min_references` 改用 `PageId` 而非标题匹配
- Bucket 2.2：`WikiPage` 新增 `created_at` / `status_changed_at` 区分时间语义
- Bucket 3：CLI / MCP 集成测试补齐
- Bucket 4.2：`PageStatusChanged` / `PageDeleted` 下游消费（bridge / MempalaceWikiSink）
- Bucket 5：Clippy / cargo audit / DomainSchema JSON Schema
- T2 整套：标签治理（`deprecated_tags` 拦截、`max_new_tags_per_ingest` 限流），依赖 `Claim` / `Source` 模型增 `tags` 字段

---

## 四、执行顺序建议

```
Step 1 (我开发, 1 天)          ← ✅ 已完成
  └─ D1 → D2 → D3 → D4
      （先上 frontmatter，再修反向规则，再顺手做 ingest-llm 默认 entry_type，最后 backup 脚本）

Step 2 (你配置, 45 分钟)       ← ✅ 已完成
  └─ U1 → U2 → U3 → U4
      （填 key → 定目录 → 改 schema → vault 指向）

Step 3 (Notion 迁移)           ← ✅ 已完成（2026-04-22）
  └─ 3 个 Notion DB → Export ZIP → wiki-migration-notion parser → ~/Documents/wiki/
  └─ 4477 条 markdown + 12804 内部边 + 4313 外部边

Step 4 (开始 dogfood)
  └─ 每周灌 3–5 篇真实文章，同步记录痛点（建议追加到 Progress.md）
  └─ 2 周后回头看：哪些 lifecycle 阈值要调？哪个 Bucket 必须补？
```

---

## 附录 A：当前 `DomainSchema.json` lifecycle 默认值（U3 参考）


| entry_type       | 初始       | 晋升路径                                                            | stale | 自动清理  |
| ---------------- | -------- | --------------------------------------------------------------- | ----- | ----- |
| concept / entity | draft    | draft→in_review（7 天 + 定义/关键要点/来源引用）→ approved（min_references=2） | 30 天  | 否     |
| summary          | approved | —                                                               | 90 天  | 否     |
| synthesis        | draft    | draft→in_review（1 天 + 4 个 section）→ approved（cooldown 3 天）      | 90 天  | 否     |
| qa               | approved | —                                                               | 90 天  | 否     |
| lint_report      | approved | —                                                               | 7 天   | **是** |
| index            | approved | —                                                               | 永久    | 否     |


**已知体感风险**：

- 第一批灌的文章互相无引用时，concept 卡在 in_review 上不去 approved（`min_references=2` 太严）
- 30 天 stale 对低频读者偏紧（配合 D2 反向规则可缓解）

---

## 附录 B：实际 vault 目录结构

> 以下为 2026-04-22 迁移完成后的实际产物，Obsidian vault 根目录 = `~/Documents/wiki/`

```
~/Documents/wiki/                ← Obsidian vault 指向这一层
├── .obsidian/                   Obsidian 配置
├── .wiki/                       迁移元数据
│   ├── uuid-map.json            Notion UUID → 相对路径（4477 条）
│   └── migration-stats.json     迁移统计
├── DomainSchema.json            生命周期规则配置
├── pages/                       Wiki 条目（按类型分子目录）
│   ├── concept/                 1448 条
│   ├── entity/                  701 条
│   ├── summary/                 1108 条（桥梁层，100% 已审核）
│   ├── synthesis/               63 条
│   ├── qa/                      5 条
│   ├── lint-report/             45 条
│   └── index/                   6 条
└── sources/                     原始文章（按来源分）
    ├── x/                       673 条 X 书签
    └── wechat/                  425 条微信文章
```

每个 `.md` 文件包含 YAML frontmatter（title / entry_type / status / tags / notion_uuid / …）+ 正文。
正文里的 Notion mention 已改写为 Obsidian 可识别的相对路径链接。

---

## 附录 C：Notion 迁移记录（已完成）

> 迁移策略：**方案 B**——三个库都迁，重建 source→synthesis 关系。
> 关系存储方式：**松关系**（Wiki 条目的 `源文章URL` 字段 + 正文内 inline 链接）。
> 执行日期：**2026-04-22**
> 状态：**✅ 完成**

### 迁移结果


| 指标                | 数值                                           |
| ----------------- | -------------------------------------------- |
| 总页解析              | **4477**（Wiki 3377 + X 674 + 微信 426），100% 成功 |
| 内部边（mention→page） | **12804**（99.6% 解析，仅 46 条未命中）                |
| 外部边 Wiki→Source   | **4313**（URL 命中 3699 + 源文章URL 字段 614）        |
| 伪 URL 清洗          | **1072**（`claude.md` 等 Notion 自动链接化 bug）     |
| 孤儿 source         | **266**（无任何 Wiki 页引用，已标 `orphan: true`）      |
| 数据丢失              | **0**                                        |
| 迁移工具耗时            | ~10 秒（离线 Rust parser，零 Notion API 调用）        |
| 产出体积              | 26 MB                                        |


### 迁移工具

- crate：`llm-wiki/crates/wiki-migration-notion`
- 子命令 `dry-run`：扫三库 → `migration-report.md` + JSONL 明细
- 子命令 `migrate`：解析 + 链接改写 + 落盘
- git commit：`2d0e9d8`

### 原始 Notion 数据库


| 别名          | 总条目  | URL                                                          |
| ----------- | ---- | ------------------------------------------------------------ |
| 📚 知识 Wiki  | 3372 | `https://www.notion.so/f98c88d3d587494a98ff92fbaf9228c4`     |
| 🐦 X书签文章数据库 | 674  | `https://www.notion.so/0d3052912a5d426c8db8903ed5bb7ddb`     |
| 微信文章数据库     | 420  | 父页面 `https://www.notion.so/164701074b6881df9f76e1095820c4b8` |


### 字段映射（Wiki DB → DomainSchema）


| Notion Wiki 字段 | 类型                      | → DomainSchema       |
| -------------- | ----------------------- | -------------------- |
| 名称             | title                   | `title`              |
| 类型             | select（7 种）             | `entry_type`（完全对齐）   |
| 状态             | status（中文）              | `status`（snake_case） |
| 标签             | multi_select（14 个主题标签）  | `tags`               |
| 置信度            | select（high/medium/low） | confidence           |
| 源文章URL         | url                     | 主源反向指针               |


### 字段映射（微信/X书签 DB → source）


| Notion 字段 | → DomainSchema                      |
| --------- | ----------------------------------- |
| Name      | `source.title`                      |
| 文章链接      | `source.url`（stable id，跨库 join key） |
| 作者        | `source.author`                     |
| 来源        | `source.origin`                     |
| 已编译到Wiki  | `compiled_to_wiki`（bool）            |
| 备注        | `source.notes`                      |


### 关系拓扑（三层桥）

```
synthesis (63 条)
  └─ mention-page → concept / summary
       ↓
concept/entity (2149 条, 78% 草稿)
  └─ 来源引用 → summary（mention-page 或纯文本）
       ↓
summary (1108 条, 100% 已审核) ← ★ 桥梁层
  └─ "原始文章信息" 块 → 原始文章 URL
  └─ "源文章URL" 字段 → Notion source UUID
       ↓
X书签 (674) + 微信 (426) = 1100 条 source
```

### 已知脏数据点

- **249 条草稿**来源引用是纯文本非 mention-page（Notion 侧"引用升级员 Agent"未做完）
- `SuperHQ` 等 `未匹配：...` 文本标注（无 URL 可抽，跳过建边）
- `summary` 里 `- 作者：X | 来源：微信 | 发布：...` **完全无链接**的老条目
- `源文章URL` 字段仅 2026-04-20 之后的新 summary 填了
- X书签 DB 的 `来源` 字段含 "微信" 选项 → 两个源库有历史重叠

### 未完成项（后续可接续）

- **266 条孤儿 source** — ✅ 全部处理完毕
  - A 类 117 条自动补链接 ✅（72 页面 + 88 条链接）
  - B1 归一化匹配后发现全部有 summary 桥梁，无需补链接 ✅
  - B2 + C = 87 条统一标记为 `compiled_to_wiki: false` ✅（未编译队列）
- **日期字段转 ISO 8601**（目前 `2026年4月11日 08:58` 逐字透传）
- **2002 条 `www.notion.so/` 未解析内链**（指向三库之外的 Notion 页面）
- **Memory Palace bridge 接入**：让本地 wiki 变成 mempalace 数据源

