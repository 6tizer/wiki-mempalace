# Dogfood 就绪清单

> 目标：**层级二**——1–2 天打磨后可作为个人知识库日常使用。
> 此清单聚焦"开始真实喂文章"前的最小必要工作，不是完整的 T1 打磨。

---

## 一、你需要配置（我替代不了）


| #   | 任务                                                                                                        | 产出位置                           | 耗时    | 状态  |
| --- | --------------------------------------------------------------------------------------------------------- | ------------------------------ | ----- | --- |
| U1  | 复制 `llm-config.example.toml` → `llm-config.toml`，填入真实 API key                                             | 代码仓库根目录（`.gitignore` 已排除）或数据目录 | 5 分钟  | ☐   |
| U2  | 选定**数据目录**（推荐 `~/wiki-mempalace/`，也可选 `~/Documents/wiki/`、iCloud / Dropbox 同步目录）                          | 任意本地路径，**不要**放代码仓库里            | 5 分钟  | ☐   |
| U3  | 把 `DomainSchema.json` 拷贝到数据目录，**按自己节奏微调** `min_age_days` / `required_sections` / `stale_days`（现有默认值见附录 A） | `<数据目录>/DomainSchema.json`     | 30 分钟 | ☐   |
| U4  | 让 Obsidian / Logseq vault 指向 `<数据目录>/wiki/`（不是 `pages/` 子目录，要整层）                                          | 编辑器内配置                         | 5 分钟  | ☐   |
| U5  | 决定是否开启 embeddings（`--vectors`）。dogfood 阶段**建议关**，只用 chat 抽取                                               | —                              | —     | ☐   |


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

