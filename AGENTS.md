# AGENTS Workflow

本文件定义 `wiki-mempalace` 的最小稳定操作流程，目标是让不同会话中的 agent 可重复执行。

> **Vault 版式约束**：所有写入 vault 的 source / summary / concept / entity 文件必须遵守
> [docs/vault-standards.md](docs/vault-standards.md)（目录、命名、frontmatter、正文骨架）。
> 引擎 `write_projection` 只维护 `pages/{entry_type}/`、`index.md`、`log.md`；
> **不要**往 `sources/` 根或 `concepts/` 根写入文件。

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
  --db wiki.db --wiki-dir wiki --sync-wiki --viewer-scope private:agent1 \
  <SUBCOMMAND> <ARGS...>
```

> 所有"写入"子命令（`ingest` / `ingest-llm` / `file-claim` / `supersede-claim` /
> `query --write-page` / `lint` / `promote` / `promote-page` / `crystallize` /
> `batch-ingest`）完成后，CLI 会**自动**调用引擎的 `save_to_repo` 与
> `flush_outbox_to_repo_with_policy`，Agent 无需手动触发持久化 / outbox flush。

## 1. Ingest

```bash
cargo run -p wiki-cli -- --db wiki.db --wiki-dir wiki --sync-wiki ingest \
  "file:///notes/a.md" "source body text" --scope private:cli
```

可选：`--vectors --llm-config llm-config.toml` 在 ingest 后写入 embedding 行（需 `[embed]`）。

要求：

- ingest 后持久化与 outbox flush 均由 CLI 自动完成。
- 若开启 `--sync-wiki`，Markdown 投影层会同步更新。

## 2. Query 与结果沉淀

```bash
cargo run -p wiki-cli -- --db wiki.db --wiki-dir wiki --sync-wiki \
  --viewer-scope private:agent1 \
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
cargo run -p wiki-cli -- --db wiki.db --wiki-dir wiki --sync-wiki lint
```

`lint` 与 `promote` / `promote-page` 同样遵循顶层 `--viewer-scope`，与 `query` 一致。

要求：

- 关注 `page.orphan`、`claim.stale`、`xref.missing`、`page.incomplete`。
- lint 报告写入 `wiki/reports/`。

## 4. Outbox 消费

导出增量事件：

```bash
cargo run -p wiki-cli -- --db wiki.db export-outbox-ndjson-from --last-id 100
```

（如需不带游标的一次性全量导出，可用 `export-outbox-ndjson`。）

消费确认：

```bash
cargo run -p wiki-cli -- --db wiki.db ack-outbox --up-to-id 120 --consumer-tag mempalace
```

桥接消费（最小实现）：

```bash
cargo run -p wiki-cli -- --db wiki.db consume-to-mempalace --last-id 100
```

## 5. Supersede 策略

- 新结论优先通过 `supersede-claim` 替换旧 claim：

  ```bash
  cargo run -p wiki-cli -- --db wiki.db \
    supersede-claim <old_claim_id> "新版结论" \
    --scope private:cli --tier semantic
  ```

- 旧 claim 被标记为过期（lint 中以 `claim.stale` 呈现）后**不删除**，保留审计与可回溯性。
- lint 中若出现 stale 相关提示，应在页面层补充新旧结论关系。
- 若只是录入一条新的独立 claim（非替换），使用 `file-claim "<text>" --scope ... --tier ...`。

## 6. LLM 结构化 ingest（可选）

```bash
cargo run -p wiki-cli -- --db wiki.db --llm-config llm-config.toml \
  ingest-llm "file:///notes/a.md" "正文……" --scope private:cli
```

`--dry-run` 仅打印模型 JSON，不落库。失败时不写入 claim（可结合日志排查）。
ingest-llm 产出的 summary page 自 M7 起固定为 `EntryType::Summary`，遵循 vault-standards
的 5 段正文骨架。

批量处理 vault 中 `compiled_to_wiki: false` 的 source：

```bash
cargo run -p wiki-cli -- --db wiki.db --wiki-dir ~/Documents/wiki --sync-wiki \
  batch-ingest --vault ~/Documents/wiki --delay-secs 1
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
