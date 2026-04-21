# wiki-mempalace 实施里程碑

本文是 `wiki-mempalace` 对齐并超越 Karpathy LLM Wiki 方案的执行版路线图。

## M1: Wiki 投影层 ✅

- CLI 支持 `--wiki-dir` 与 `--sync-wiki`，将结构化状态投影到 markdown。
- 自动生成 `index.md`、`log.md`，并维护 `pages/`、`concepts/`、`sources/`。
- Query 支持 `--write-page`，将回答结果沉淀为 wiki 页面。
- **D1 完成**：YAML frontmatter 投影（id / status / entry_type / updated_at）。

验收标准（已达成）：

- 连续 ingest 后，`index.md` 和 `log.md` 都产生增量。
- query 结果可选择落盘并出现在 `index.md` 中。

## M2: 一致性与消费语义 ✅

- lint 增强：`page.orphan`、`claim.stale`、`xref.missing`、`page.incomplete`（完整度）。
- 产出 lint 报告文件到 `wiki/reports/`。
- outbox 增加游标导出与处理确认：
  - `export-outbox-ndjson-from --last-id`
  - `ack-outbox --up-to-id --consumer-tag`
- outbox flush 改为分批 + 重试策略。

验收标准（已达成）：

- lint 结果可直接用于 wiki 修复。
- consumer 可以 offset 重放并标记消费进度。

## M3: mempalace 联动与标准化流程 ✅

- 提供 `consume-to-mempalace --last-id` 最小消费器。
- bridge 提供 `consume_outbox_ndjson` 事件分发能力。
- 增加 `AGENTS.md` 规范新会话可重复执行。
- **Phase 6a 完成**：mempalace_* MCP 工具统一走 bridge 抽象。

验收标准（已达成）：

- 演示 ingest -> outbox -> consume -> query -> file back 全流程。
- 关键操作不依赖隐式记忆，按规范文档可复现。

## M4: Notion 数据迁移 ✅

- 离线 Rust parser（`wiki-migration-notion` crate）处理 Notion Export ZIP。
- 三个 Notion DB 全量迁移：知识 Wiki（3377）+ X书签（674）+ 微信（426）= 4477 条。
- 内部边 12804（99.6% 解析），外部边 4313（Wiki→Source），伪 URL 清洗 1072。
- 落盘到 `~/Documents/wiki/`，Obsidian 验证通过。

验收标准（已达成）：

- 三库全量迁移，零数据丢失。
- Obsidian 可浏览、内部边可点击、搜索正常。

## M5: Dogfood 就绪 ✅

- D1–D4 全部完成（frontmatter / 反向 promotion / 默认 entry_type / 备份脚本）。
- U1–U5 全部完成（API key / 数据目录 / Schema / Obsidian vault / embeddings）。
- Schema T0 + T1 闭环（status / promote / stale / cleanup / validate）。
- 62 个测试全绿，E2E 脚本通过。

## 后续（未开始）

- 266 条孤儿 source 审计
- 日期字段转 ISO 8601
- `www.notion.so/*` 未解析内链处理
- Memory Palace bridge 接入（mempalace 消费迁移产物）
- T2 标签治理