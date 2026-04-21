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
| U4  | 让 Obsidian / Logseq vault 指向 `<数据目录>/wiki/`（不是 `pages/` 子目录，要整层）                                          | 编辑器内配置                         | 5 分钟  | ☐   |
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
| D1  | Bucket 4.1 | **projection 写 YAML frontmatter**（`id` / `status` / `entry_type` / `updated_at`），修改 `crates/wiki-kernel/src/wiki_writer.rs`，配套测试 | **决定性**——否则 Obsidian 里看不到 lifecycle 状态，T1 成果对用户完全不可见 | 半天     | ☐   |
| D2  | Bucket 1.2 | 在 `DomainSchema.json` 为 concept/entity 加反向 promotion 规则 `needs_update → approved`（否则页面 stale 后锁死）                                | 关键修复                                                 | 1 小时   | ☐   |
| D3  | 新增         | `ingest-llm` 不传 `--entry-type` 时默认 `concept`（或让 LLM 在 plan 里判断并写回 `LlmIngestPlanV1`）                                             | 每次 ingest 不用手动指定                                     | 1–2 小时 | ☐   |
| D4  | 新增         | `scripts/backup.sh`：cp SQLite db 到 `<数据目录>/backups/knowledge-YYYYMMDD-HHMM.db`                                                   | 防丢数据                                                 | 15 分钟  | ☐   |


**合计：约 1 天**。

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
Step 1 (我开发, 1 天)
  └─ D1 → D2 → D3 → D4
      （先上 frontmatter，再修反向规则，再顺手做 ingest-llm 默认 entry_type，最后 backup 脚本）

Step 2 (你配置, 45 分钟)
  └─ U1 → U2 → U3 → U4
      （填 key → 定目录 → 改 schema → vault 指向）

Step 3 (开始 dogfood)
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

## 附录 C：Notion 源数据库清单（迁移阶段参考）

> 迁移策略：**方案 B**——三个库都迁，重建 source→synthesis 关系。
> 关系存储方式：**松关系**（Wiki 条目的 `源文章URL` 字段 + 正文内 inline 链接）。

### 数据库


| 别名          | 总条目  | URL                                                                                                                         | data-source id                                      |
| ----------- | ---- | --------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| 📚 知识 Wiki  | 3372 | `https://www.notion.so/f98c88d3d587494a98ff92fbaf9228c4`                                                                    | `collection://0a096c18-fcbf-4f47-8e96-7deb4d637da3` |
| 🐦 X书签文章数据库 | 674  | `https://www.notion.so/0d3052912a5d426c8db8903ed5bb7ddb`                                                                    | `collection://f5991049-e6aa-4eeb-b3ba-082ea44106d8` |
| 微信文章数据库     | 420  | 父页面 `https://www.notion.so/164701074b6881df9f76e1095820c4b8`，内嵌 DB `https://www.notion.so/164701074b68810abc81f90795cc29ad` | `collection://16470107-4b68-812d-b409-000b71a2be7f` |


**注意**：X书签 DB 的 `来源` 字段里含 "微信" 选项，说明两库可能有历史重叠，迁移时按 `文章链接` URL 去重。

### 字段映射（Wiki DB → DomainSchema）


| Notion Wiki 字段         | 类型                                                            | → DomainSchema                                             |
| ---------------------- | ------------------------------------------------------------- | ---------------------------------------------------------- |
| 名称                     | title                                                         | `title`                                                    |
| 类型                     | select（concept/summary/synthesis/entity/index/lint-report/qa） | `entry_type`（7 种**完全对齐**，注意 `lint-report` → `lint_report`） |
| 状态                     | status（草稿/审核中/已审核/需更新）                                        | `status`（draft/in_review/approved/needs_update）            |
| 标签                     | multi_select（14 个主题标签）                                        | `tags`                                                     |
| 置信度                    | select（high/medium/low）                                       | confidence                                                 |
| 源文章URL                 | url                                                           | 主源反向指针（单源）                                                 |
| 来源标签                   | text                                                          | 从源透传的细粒度标签                                                 |
| 创建时间 / 最后编辑时间 / 最后编译时间 | time                                                          | metadata                                                   |


### 字段映射（微信/X书签 DB → DomainSchema source）


| Notion 字段 | 类型                         | → DomainSchema                          |
| --------- | -------------------------- | --------------------------------------- |
| Name      | title                      | `source.title`                          |
| 文章链接      | url                        | `source.url`（**stable id**，跨库 join key） |
| 作者        | text                       | `source.author`                         |
| 来源        | select（微信/X书签/哔哩哔哩）        | `source.origin`                         |
| 标签        | multi_select               | `source.tags`                           |
| 备注        | text                       | `source.notes`（含 LLM 预抽取摘要）             |
| 发布时间      | date                       | `source.published_at`                   |
| 已编译到Wiki  | checkbox                   | 审计标记（`__YES_`_/`__NO__`）                |
| 状态        | select（待处理/已提取/已写/已发布/已完成） | source lifecycle                        |


### 关系拓扑（抽样后修正）

实际不是两层，而是**三层桥**：

```
synthesis (62 条, 59 审核中)
  └─ 来源列表（mention-page 规范）→ concept / summary pages
       ↓
concept/entity (2144 条, 78% 草稿)
  └─ 来源引用 → summary pages（12.5% mention-page）
                  或纯文本：《文章标题》｜文章链接：[URL](URL)（87.5%）
       ↓
summary (1105 条, 全部已审核) ← ★ 桥梁层
  └─ "原始文章信息" 块 → 原始文章 URL
  └─ "源文章URL" 字段 → Notion X书签/微信 DB 页面 URL（仅新条目填了）
       ↓
X书签 DB (674) + 微信 DB (420) = 1094 条 source
```

### 全库真实状态（截自 2026-04-21 Lint Report）


| 类型        | 草稿       | 审核中     | 已审核      | 需更新   | 合计       | 备注                               |
| --------- | -------- | ------- | -------- | ----- | -------- | -------------------------------- |
| concept   | 1079     | 347     | 17       | 1     | 1444     | 78% 草稿                           |
| entity    | 487      | 211     | 2        | 0     | 700      | 70% 草稿                           |
| summary   | 0        | 0       | **1105** | 0     | 1105     | **100% 已审核，最干净**                 |
| synthesis | 3        | 59      | 0        | 0     | 62       | 95% 审核中                          |
| qa        | 1        | 0       | 4        | 0     | 5        | —                                |
| **总计**    | **1570** | **617** | **1128** | **1** | **3316** | Lint Report 未计 lint-report/index |


### 正文链接格式清单（用于正则抽取）

1. **Notion 内部 mention**：`<mention-page url="https://www.notion.so/{uuid32}"/>`
2. **纯文本引用（concept/entity 主力）**：`- 《{标题}》｜文章链接：[{url}]({url})`
3. **Markdown link**：`[{锚文本}]({url})` 或 `[文章链接]({url})`
4. **裸 URL**：`https://x.com/...`、`https://mp.weixin.qq.com/...`、`https://t.co/...`
5. **未匹配标注**：`- 未匹配：...`（保留为文本，不建边）

### 迁移 Pipeline（v1 设计）

**不走 Notion API 分页**——3372 条 × fetch 约 18 分钟且容易触限。走 **Notion 官方 Export** 一次性导出：

```
Step 1: Notion 手动导出 → Markdown + CSV ZIP（3 个 DB 各导一次）
Step 2: 离线 parse（Python 或 Rust 脚本）
  2.1 读 CSV 拿元数据（title / entry_type / status / tags / source_url / notion_uuid）
  2.2 解析 Markdown body
      - 抽 <mention-page> UUID → 记为内部边
      - 抽 纯文本 [url](url) → 记为外部边（待 resolve）
      - 抽 裸 URL 正则
Step 3: 写入 Wiki-mempalace
  3.1 先写 1094 条 source（WeChat + X书签）——按 文章链接 URL 去重
  3.2 写 1105 条 summary（已审核数据最稳）
  3.3 写 2144 条 concept/entity（草稿继续维持 draft 状态）
  3.4 写 62 条 synthesis
Step 4: 建关系边
  4.1 内部边：通过 Notion UUID → 本地 page_id 映射表 resolve
  4.2 外部边：通过 URL 匹配 resolve 到 source
  4.3 未 resolve 的保留为 "未匹配" 文本标注
Step 5: 审计
  - 对比 Source.已编译到Wiki=true 与实际被引用集合 → 孤儿 source
  - 对比 concept/entity 无任何 source 引用 → 孤儿 wiki
```

**预期处理时长**：离线脚本跑一次约 10-30 分钟（取决于正则复杂度），一次性完成。

### 导出产物（已落盘，2026-04-22 07:07）


| 库    | 源 ZIP                                                                                                               | 解压路径                                    | md 文件数 |
| ---- | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ------ |
| Wiki | `/Users/mac-mini/Library/Mobile Documents/com~apple~CloudDocs/Downloads/56304c5d-...%2FExportBlock-5472a6c2-...zip` | `/tmp/notion-wiki-clean/私人与共享/知识 Wiki/` | 3377   |
| X书签  | `/Users/mac-mini/Downloads/2eed1dfc-...ExportBlock-b776a17a-...zip`                                                 | `/tmp/notion-db2/私人与共享/X书签文章数据库/`       | 674    |
| 微信   | `/Users/mac-mini/Downloads/81a692b9-...ExportBlock-178a84c9-...zip`                                                 | `/tmp/notion-db3/私人与共享/微信文章数据库/文章数据库/`  | 423    |


**关键发现**：三个库导出格式**完全同构**：`# 标题` + `Key: value` 属性块 + 空行 + Markdown 正文。单一 parser 即可处理。

**Source CSV 列**（X / 微信 共有）：`Name, 作者, 创建时间, 发布时间, 备注, 已编译到Wiki, 文章链接, 来源, 标签, 状态`。
X 额外含 `公众号状态`。

**文件名 UUID 约定**：所有文件名末尾带 32 位 hex UUID（如 `Gemma 4 09a31eaf99cc4161b51e7029278bc78e.md`），这是 **Notion 内部 page ID**，也是跨库 join 的主要 key（结合 `文章链接` URL 做二级 join）。

**macOS 解压注意**：系统自带 `unzip` 无法处理中文文件名（Illegal byte sequence），必须用 `ditto -x -k`。

### 已知脏数据点（迁移时要处理）

- **249 条草稿**（2026-04-11 ~ 13 批次）来源引用是纯文本非 mention-page（Notion 侧"引用升级员 Agent"未做完）
- `SuperHQ` 类型 `未匹配：...` 文本标注（无 URL 可抽，跳过建边）
- `summary` 里 `- 作者：X | 来源：微信 | 发布：...` **完全无链接**的老条目（保留元数据，建不了边）
- `源文章URL` 字段仅 2026-04-20 之后的新 summary 填了（老数据得靠正文解析）
- X书签 DB 的 `来源` 字段含 "微信" 选项 → 两个源库可能有 URL 重叠，按 URL 强去重

---

## 附录 B：wiki-dir 目录结构（U4 参考）

```
<数据目录>/wiki/          ← Obsidian vault 指向这一层
├── index.md              总览入口
├── log.md                审计日志（crystallize / promote / supersede）
├── pages/                WikiPage（主力阅读）
├── concepts/             Claim（原子事实）
├── sources/              原始 Source 预览
├── analyses/             query --write-page 产物
└── reports/              lint 报告（7 天自动清）
```

