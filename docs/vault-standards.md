# Vault 唯一标准（Notion 对齐）

本文档是 **wiki vault**（Markdown 文件树 + `wiki.db` 引擎）的单一事实来源：**Notion 迁移产出的版式即标准**。所有来源（Notion 迁移、`batch-ingest`、未来抓取工具）必须遵守同一套目录、命名与 frontmatter 契约。

## 目录布局


| 路径                    | 含义                                                                          |
| --------------------- | --------------------------------------------------------------------------- |
| `sources/{origin}/`   | 原始文章；`origin ∈ {wechat, x, manual, …}`。**禁止**在 `sources/` 根目录直接放置 `.md` 文件。 |
| `pages/summary/`      | 单源导读：一篇 source 对应一篇 summary。                                                |
| `pages/concept/`      | `entry_type: concept` 的页面。                                                  |
| `pages/entity/`       | `entry_type: entity`。                                                       |
| `pages/synthesis/`    | `entry_type: synthesis`。                                                    |
| `pages/qa/`           | `entry_type: qa`。                                                           |
| `pages/index/`        | `entry_type: index`。                                                        |
| `pages/lint-report/`  | `entry_type: lint_report`。                                                  |
| `pages/_unspecified/` | 引擎投影时 `entry_type` 为空的页面（应避免长期停留）。                                          |
| `reports/`            | Lint、governance、fixer、synthesis、scheduled bundle 等报告。                          |
| `reports/governance/` | `governance scan` JSON / Markdown。                                          |
| `reports/fixer/`      | Evidence Fixer plan/apply/restore/tombstone artifacts。                       |
| `reports/synthesis/`  | Synthesis discovery / compose artifacts。                                     |


`write_projection` **只**维护 `pages/`**、`index.md`、`log.md`；**不向** `sources/` 根目录写 source 副本，**也不向** 根 `concepts/` 写哈希命名的 claim 投影（claim 的语义由 `pages/concept/` 承载，引擎内部保留在 `wiki.db`）。

引擎投影页必须带：

- `managed_by: wiki-mempalace`
- `managed_kind: wiki-page-projection`

只有同时带上述 marker 和合法 UUID `id:` 的 `pages/` 文件会被 projection cleanup 视为
engine-managed。stale / obsolete managed page 会被移动到 `.wiki/trash/projection/`，
不会直接不可恢复删除。手写页面即使有 UUID `id:`，只要没有 managed marker，也不会被
`write_projection` 清理或覆盖；若它与新 projection 目标同名，projection 会失败并保留
原文件。`pages/` 和 `pages/{entry_type}/` 写入父目录不能是 symlink；root
`index.md` / `log.md` 也拒绝 symlink 目标。

## 命名规则

- **Notion 迁移 slug**：保留中文字符；空白、标点、`/` 等折叠为 `-`；文件名最长 80 个字符；**不用** UUID 作为默认文件名。
- 引擎投影时如果标题 slug 为空，文件名 fallback 为 `page-{page_id前8位}.md`，避免生成 `.md`。
- Summary 文件：`pages/summary/摘要：{原标题}.md`（与 Notion 迁移一致）。

## Source frontmatter 契约

每条 `sources/{origin}/*.md` 至少应包含：

- `title`（必填）
- `kind: source`
- `origin`（如 `wechat` / `manual`）
- `url`（可空）
- `origin_label`、`published_at`、`notes`（按来源填写）
- `compiled_to_wiki: true|false`
- `orphan`（可选）
- `created_at`
- `tags`（可选，可为逗号分隔字符串或后续统一为列表）

从 Notion 导入时：`notion_uuid` **必填**。`batch-ingest` 新建的 source 可为空。

## Summary frontmatter 契约

`pages/summary/*.md`：

- `title`：与正文 H1 一致，形如 `摘要：{原标题}`
- `entry_type: summary`
- `status`：与 `DomainSchema` / `initial_status_for` 一致（如 `approved`）
- `confidence`：`high` | `medium` | `low`（来自 LLM 计划，缺省为 `medium`）
- `source_url`：回填自 source 的 `url` 或 `file://` URI
- `source_tags`：来自 source frontmatter 的标签列表
- `tags`：LLM 抽取的 wiki 标签
- `created_at`：与 source 的 `created_at` 对齐（缺省则填编译时刻）
- `updated_at`、`last_compiled_at`：编译时刻（RFC3339）
- `compiled_by`：如 `batch-ingest`

## Summary 正文骨架（5 段，不可省略）

固定二级标题，顺序如下；无内容时写 **「（暂无）」**：

1. `## 一句话摘要`
2. `## 关键洞察`
3. `## 提取的概念`（通常由 claims / 结构化抽取呈现）
4. `## 原始文章信息`（链接、作者、平台、时间等）
5. `## 个人评注`（机器流水线默认为「（暂无）」）

正文最外层可有 `# 摘要：{原标题}` 与 frontmatter 的 `title` 对齐。

## Concept / Entity 页面契约

Wiki Compiler 产出的重要概念和实体必须成为可见页面，而不是只停留在
`wiki.db` 的内部结构里。

`pages/concept/*.md`：

- `entry_type: concept`
- `status: draft`
- 用于技术概念、方法论、架构模式、协议、非显然思想。
- 正文至少包含：
  - `## 定义`
  - `## 关键要点`
  - `## 本文语境`
  - `## 来源引用`

`pages/entity/*.md`：

- `entry_type: entity`
- `status: draft`
- 用于具体产品、公司、人物、项目、库、模型、工具。
- 正文至少包含：
  - `## 定义`
  - `## 关键要点`
  - `## 来源引用`

引用约束：

- summary 的 `## 提取的概念` 段应使用 `[[页面标题]]` 链接 concept/entity。
- concept/entity 的 `## 来源引用` 段应反链对应 summary。
- 新增引用前必须按 summary 标题 / page id / source url 去重，不重复追加同一来源。

## Synthesis 页面契约

`pages/synthesis/*.md`：

- `entry_type: synthesis`
- `status: in_review`
- `confidence: high`
- 由 `research-synthesis compose/run` 写入时必须来自已通过 verifier 的 draft。
- 正文固定包含：
  - `## 研究问题`
  - `## 综合分析`
  - `## 关键发现`
  - `## 来源列表`
  - `## 外部证据`
  - `## 行动建议`

引用约束：

- 每个关键发现必须有内部 page/source evidence 或外部 evidence 支撑。
- 外部 evidence 必须能追溯到 provider、url、domain、retrieved_at 和 content_hash。
- Synthesis 只能通过 `wiki.db` 写入；不要手工在 `pages/synthesis/` 下新建投影页来绕过 verifier。

---

## 未来新增 source 的流程

### 微信公众号文章

1. 放入 `sources/wechat/`。
2. 使用标准 source frontmatter（见上文），`compiled_to_wiki: false`。
3. 运行 `wiki-cli batch-ingest`（配置好 `--wiki-dir` 与 `--db`）：管线会调用 Wiki Compiler，更新引擎，写 `pages/summary/` 与必要的 `pages/concept/` / `pages/entity/`，并把对应 source 标记为 `compiled_to_wiki: true`。

### X（Twitter）等

1. 放入 `sources/x/`（或约定的 `origin` 子目录），字段与 wechat 类似。

### 手写笔记

1. 放入 `sources/manual/`。
2. 最少字段：`title`、`kind: source`、`origin: manual`、`compiled_to_wiki: false`、`created_at`。
3. 同样由 `batch-ingest` 发现并编译。

### 禁止事项

- 不要在 `sources/` **根目录**堆积抓取结果或投影文件。
- 不要手工改 `wiki.db` 与 vault 双份状态而不跑同步/编译；重编译前应按运维流程将 `compiled_to_wiki` 置回 `false` 并清理旧 summary（见运维文档或团队 runbook）。
