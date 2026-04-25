# AGENTS Workflow

本文件定义 `wiki-mempalace` 的最小稳定操作流程，目标是让不同会话中的 agent 可重复执行。

> **Vault 版式约束**：所有写入 vault 的 source / summary / concept / entity 文件必须遵守
> [docs/vault-standards.md](docs/vault-standards.md)（目录、命名、frontmatter、正文骨架）。
> 引擎 `write_projection` 只维护 `pages/{entry_type}/`、`index.md`、`log.md`；
> **不要**往 `sources/` 根或 `concepts/` 根写入文件。
> 报告类输出（dashboard / suggest / metrics / automation health）在传入
> `--wiki-dir` 时，相对输出路径按 vault 相对路径解析，默认写入
> `<wiki-dir>/reports/`。

## 开发流程约束

后续功能开发必须按 [docs/dev-workflow.md](docs/dev-workflow.md) 执行：

1. 先有 PRD：`docs/prd/<batch>.md`。
2. PRD 后先做白话架构对话，用户确认模块职责和模块关系后再写 spec。
3. 每个模块先有 spec 三件套：
   - `docs/specs/<feature>/requirements.md`
   - `docs/specs/<feature>/design.md`
   - `docs/specs/<feature>/tasks.md`
4. spec 是实现源；spec 和代码冲突时，先改 spec，再改代码。PRD 范围变更必须让用户决定。
5. 新开发必须开 `codex/<topic>` 分支，不直接在 `main` 开发。
6. 进入实现前先走 Plan mode，明确 task grade、owner files、测试、review gate。
7. 复杂功能模块优先用 sub agent；简单机械任务用 script，固定模式任务用 skill。
8. sub agent 必须有明确写入范围，多个 sub agent 不得同时拥有同一写入范围。
9. sub agent 完成后必须写 `docs/handovers/<feature>/<module>.md`。
10. 每个小模块完成后做 focused review；全部完成后做 integration review；PR 后再做 GitHub/Codex review。
11. 合并前必须回填 PRD/spec/roadmap 状态，合并后更新 `docs/LESSONS.md`。

## 0. 命令行参数约定

`wiki-cli` 的多数开关是**顶层 global 参数**，必须出现在子命令**之前**，否则 clap 会报
`unexpected argument`。属于 global 的参数包括：

- `--db <PATH>`（默认 `wiki.db`）
- `--schema <PATH>`
- `--wiki-dir <PATH>` + `--sync-wiki`（启用 Markdown 投影）
- `--viewer-scope <SCOPE>`（检索 / lint / promote 的视角，默认 `private:cli`）
- `--vectors` + `--llm-config <PATH>`（启用向量检索/嵌入，默认 `llm-config.toml`）
- `--graph-extras-file <PATH>`

标准形式：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  <SUBCOMMAND> <ARGS...>
```

共享 vault-local 默认路径：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  <SUBCOMMAND> <ARGS...>
```

MCP 日常启动：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  mcp
```

MCP 写工具未显式传 `scope` 时使用 `--viewer-scope`；需要私域时显式传
`scope: private:<agent>`。

> 所有"写入"子命令（`ingest` / `ingest-llm` / `file-claim` / `supersede-claim` /
> `query --write-page` / `lint` / `promote` / `promote-page` / `crystallize` /
> `batch-ingest`）完成后，CLI 会**自动**调用引擎的 `save_to_repo` 与
> `flush_outbox_to_repo_with_policy`，Agent 无需手动触发持久化 / outbox flush。

## 1. Ingest

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  ingest "file:///notes/a.md" "source body text" --scope shared:wiki
```

可选：`--vectors --llm-config llm-config.toml` 在 ingest 后写入 embedding 行（需 `[embed]`）。

要求：

- ingest 后持久化与 outbox flush 均由 CLI 自动完成。
- 若开启 `--sync-wiki`，Markdown 投影层会同步更新。

## 2. Query 与结果沉淀

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  query "what changed?" --write-page --page-title "analysis-change-log"
```

可选：`--viewer-scope private:<agent>` 或 `shared:<team>` 限定检索视角；`--vectors`
启用余弦向量路（需 `[embed]`）；`--graph-extras-file path.txt` 合并外部图候选 doc id
进第三路。

要求：

- 默认返回 top ranked docs。
- 若 `--write-page`，将 query 结果写入 wiki 页面，并更新 `index.md`。

## 3. Lint 与健康检查

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  lint
```

`lint` 与 `promote` / `promote-page` 同样遵循顶层 `--viewer-scope`，与 `query` 一致。

要求：

- 关注 `page.orphan`、`claim.stale`、`xref.missing`、`page.incomplete`。
- lint 报告写入 `wiki/reports/`。

## 4. Outbox 消费

导出增量事件：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  export-outbox-ndjson-from --last-id 100
```

（如需不带游标的一次性全量导出，可用 `export-outbox-ndjson`。）

消费确认：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  ack-outbox --up-to-id 120 --consumer-tag mempalace
```

桥接消费（最小实现）：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  --viewer-scope shared:wiki \
  --palace /Users/mac-mini/Documents/wiki/.wiki/palace.db \
  consume-to-mempalace --last-id 100
```

## 5. Supersede 策略

- 新结论优先通过 `supersede-claim` 替换旧 claim：

  ```bash
  cargo run -p wiki-cli -- \
    --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
    --viewer-scope shared:wiki \
    supersede-claim <old_claim_id> "新版结论" \
    --scope shared:wiki --tier semantic
  ```

- 旧 claim 被标记为过期（lint 中以 `claim.stale` 呈现）后**不删除**，保留审计与可回溯性。
- lint 中若出现 stale 相关提示，应在页面层补充新旧结论关系。
- 若只是录入一条新的独立 claim（非替换），使用 `file-claim "<text>" --scope ... --tier ...`。

## 6. LLM 结构化 ingest（可选）

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki --llm-config llm-config.toml \
  ingest-llm "file:///notes/a.md" "正文……" --scope shared:wiki
```

`--dry-run` 仅打印模型 JSON，不落库。失败时不写入 claim（可结合日志排查）。
ingest-llm 产出的 summary page 自 M7 起固定为 `EntryType::Summary`，遵循 vault-standards
的 5 段正文骨架。

批量处理 vault 中 `compiled_to_wiki: false` 的 source：

```bash
cargo run -p wiki-cli -- \
  --db /Users/mac-mini/Documents/wiki/.wiki/wiki.db \
  --wiki-dir /Users/mac-mini/Documents/wiki --sync-wiki \
  --viewer-scope shared:wiki \
  batch-ingest --vault /Users/mac-mini/Documents/wiki --delay-secs 1
# 可选：--dry-run 只扫描不编译；--limit N 限制处理条数。
```

## 7. 维护类命令（按需）

以下子命令不在日常流水线里，但 Agent 需要时可以直接调用：

- `promote <claim_id>` — 对满足条件的 claim 手动推进生命周期。
- `promote-page <page_id> [--to <status>] [--force]` — 推进页面状态（`Draft → InReview → Approved`）。
- `crystallize "<question>" --finding ... --file ... --lesson ...` — 会话结晶到一张新页面。
- `maintenance` — 批量执行 confidence decay + lint + 自动 promote 合格 claim。
- `schema-validate [path]` — 校验 `DomainSchema.json`（默认仓库根的该文件），不需要 DB。
- `llm-smoke --config llm-config.toml` — 对 LLM 配置做一次最小连通性测试。
