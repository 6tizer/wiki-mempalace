# wiki-mempalace 工作进度

按用户规范逐步记录每一次有意义的工作。每条记录包括：实现的功能 / 遇到的错误 / 解决方式。

> 仓库改名：`llm-wiki` → `wiki-mempalace`（2026-04-21 合并）；早期记录保留原名以还原上下文。

---

## 2026-04-21 · 两仓合并为 monorepo（第 3 轮）

### 背景

`llm-wiki` 与 `rust-mempalace` 两仓经过多轮迭代，已通过 `path` 依赖紧密耦合
（`wiki-cli` 和 `wiki-mempalace-bridge` 都引用 `../../../rust-mempalace`），但仓库
边界仍按"独立维护、契约联动"的原设计。架构讨论后判定：自用、无外部消费者、API
仍在漂移期——合并 monorepo 的收益显著高于保留分仓。

### 实现了哪些功能

1. **gh 创建新仓**：`gh repo create 6tizer/wiki-mempalace --private`。
2. **subtree 嫁接**：用 `git subtree add --prefix=crates/rust-mempalace` 把
  `rust-mempalace` 整棵树带 4 个历史 commit 嫁接到 `llm-wiki` 的工作副本，保留
   完整 blame 能力；合并后 log 可见 llm-wiki 5 个 + rust-mempalace 4 个 + subtree
   merge commit 共 10 条。
3. **workspace 接纳**：根 `Cargo.toml` 的 `[workspace] members` 追加
  `"crates/rust-mempalace"`；`wiki-cli` 与 `wiki-mempalace-bridge` 的 path 依赖
   从 `../../../rust-mempalace` 改为 `../rust-mempalace`。
4. **Edition 策略**：采用方案 A，`crates/rust-mempalace/Cargo.toml` 保留独立
  `edition = "2024"`，不继承 workspace 的 `2021`。Cargo 允许 member override，
   零代码改动。
5. **CI workflow 迁移**：原 `crates/rust-mempalace/.github/workflows/{ci-quick,
  ci-e2e}.yml`移到仓库根`.github/workflows/`，改名为` ci-mempalace-*.yml`，  增加` paths: ['crates/rust-mempalace/**', '.github/workflows/ci-mempalace-*.yml']  `过滤器；`cargo test`命令加`-p rust-mempalace` 定位 workspace 中的目标 crate。
6. **文档梳理**：
  - 根 `README.md` 重写为"统一产品"视角
  - 新增 `docs/architecture.md` 落盘架构图与 ingest / query 业务流转
  - `docs/mempalace-linkage.md` 改写为"workspace 内 crate 边界契约"
  - `article2.md` 移到 `docs/blog/article2.md`（历史长文归档）
  - `crates/rust-mempalace/docs/longmemeval.md` 上移到顶层 `docs/longmemeval.md`
  - `crates/rust-mempalace/README.md` 保留，作为 crate 级独立说明

### 遇到了哪些错误

1. **subtree 带入孤儿 `Cargo.lock`**：`crates/rust-mempalace/Cargo.lock` 与
  workspace 根 `Cargo.lock` 冲突。
2. **rust-mempalace 原 CI 的 `cargo test --bin rust-mempalace` 在 workspace 模式
  可能跑错包**（所有 binary crate 都会被索引）。

### 是如何解决这些错误的

1. `rm crates/rust-mempalace/Cargo.lock`，workspace 只保留根 lock。
2. 迁移后的 workflow 将命令改为 `cargo test -p rust-mempalace --bin rust-mempalace`，
  用 `-p` 明确包名，避免误匹配。

### 验证

- `cargo check --workspace`：6 个 crate 全部干净编译。
- `cargo test --workspace`：**49 个测试全绿**（比合并前 40 多 8 个 e2e_core + 1
个 rust-mempalace bin test）。
- `cargo test -p rust-mempalace --test e2e_core`：8 用例 PASS。
- `./scripts/e2e.sh`：9 步全部 PASS，consumed=4。
- `git log --oneline --all`：10 个 commit，双方历史完整。
- `git log --oneline crates/rust-mempalace/` 能独立回溯原 rust-mempalace 历史。

### 未完成项（Phase 6 后续 PR 跟进）

- **Phase 6a**：`wiki-cli/src/mcp.rs` 的 10 个 `mempalace`_* MCP 工具改走 bridge
抽象，消除 `crates/wiki-cli/Cargo.toml` 对 `rust-mempalace` 的直接 path 依赖。
- **Phase 6b**：workspace 整体升 `edition = "2024"`，让所有 crate 对齐。
- 原仓库 `6tizer/llm-wiki` 与 `6tizer/rust-mempalace` 按决策保持**原样不动**，
不 archive 不删除。

---

## 2026-04-21 · Schema 审阅与闭环修复（第 1 轮）

### 背景

`crates/wiki-core/src/schema.rs` 新增了 235 行（`EntryType` / `EntryStatus` / `LifecycleRule` /
`PromotionRule` / `TagConfig` / `CompletenessConfig`），并新增了仓库根的
`DomainSchema.json`（Schema v1.0 实例）。审阅后定位到两个阻塞点并做最小闭环修复。

### 实现了哪些功能

1. **修复 `DomainSchema.json` 使其可被反序列化**：将所有枚举字面量
  （`allowed_entity_kinds`、`allowed_relations`、`tier_half_life_days` 键，
   `lifecycle_rules[].entry_types`、`initial_status`、`promotions[].from_status/to_status`）
   从 PascalCase 统一改写为 snake_case，对齐 `#[serde(rename_all = "snake_case")]`。
2. `**crates/wiki-core/src/lib.rs` 补齐 re-export**：新增
  `CompletenessConfig / EntryStatus / EntryType / LifecycleRule / PromotionConditions /  PromotionRule / TagConfig` 的 public re-export，让下游 crate 和 CLI 能够引用这些类型。
3. **新增 `crates/wiki-core/tests/schema_json.rs` 契约测试（3 个用例）**：
  - `repo_domain_schema_json_deserializes`：仓库根 `DomainSchema.json` 可被
   `from_json_path` 加载并校验字段数量。
  - `repo_domain_schema_lifecycle_rules_indexable`：通过
  `find_lifecycle_rule` 按 `EntryType` 索引生命周期规则，验证 snake_case 别名如
  `lint_report` 解析正确。
  - `round_trip_serialize_deserialize`：`serde_json::to_vec` → `from_json_slice` 可来回
  转换，防止未来加字段漏加 `#[serde(default)]`。

### 遇到了哪些错误

1. **枚举字面量大小写失配导致 JSON 加载失败**：
  ```
   Error: Json(Error("unknown variant `Person`, expected one of
     `person`, `project`, `library`, `concept`, `file_path`, `decision`, `other`", line: 4, column: 10))
  ```
   根因：`model.rs` 的 `EntityKind / RelationKind / MemoryTier` 与新增的 `EntryType /  EntryStatus` 都使用 `#[serde(rename_all = "snake_case")]`，但 JSON 中仍是 PascalCase。
2. **第一次 `StrReplace` 返回 "not found" 假告警**：实际修改已经成功落盘，第二次重复
  修改时自然找不到原字符串。通过 `Read` 确认文件已更新，忽略该告警并继续。

### 是如何解决这些错误的

1. 将 JSON 里 6+7+4+6+多处枚举值全部改成 snake_case（`Person → person`、
  `DependsOn → depends_on`、`Working → working`、`InReview → in_review`、
   `LintReport → lint_report` 等），保持代码不动。
2. 通过 `Read` 工具确认文件实际状态；后续遇到类似"先改又触发改"时，先读再判断。

### 验证

- `cargo test -p wiki-core`：10 个单测 + 3 个 `schema_json` 集成测试全通过。
- `cargo check --workspace`：5 个 crate 均通过，无编译告警。
- 端到端回归：
`cargo run -p wiki-cli -- --db /tmp/schema_check.db --schema DomainSchema.json lint`
成功启动并完成，无反序列化错误。

---

## 2026-04-21 · Schema 硬化与 lint 接入（第 2 轮）

### 背景

第 1 轮只是让 JSON 能加载，但宪法 DSL 仍缺少执行者。本轮完成：
`parse()` 严格化、`validate()` 语义校验、去业务耦合、lint 接入 `completeness_config`、
`PromotionConditions.required_sections` 与 `CompletenessConfig.*_required_sections`
职责边界注释化。

### 实现了哪些功能

1. `**EntryType::parse` / `EntryStatus::parse` 改为 `Result<Self, SchemaValidationError>`**：
  未知输入不再静默回落到 `Draft` / `Concept`，避免拼写错误被静默吃掉。
2. `**SchemaValidationError` 与 `SchemaLoadError::Invalid(..)`**：新增语义错误枚举，
  覆盖重复 EntryType / promotion 自环 / 环路 / 未知字面量 4 种情况。
3. `**DomainSchema::validate()`**：
  - 规则 1：任一 `EntryType` 最多出现在一条 `LifecycleRule.entry_types` 中；
  - 规则 2：任一 `PromotionRule` 的 `from_status != to_status`；
  - 规则 3：每条 rule 的 promotions 形成的有向图无环（三色标记 DFS）。
   `from_json_slice` / `from_json_path` 在返回前自动调用 `validate`，bad schema 快速失败。
4. `**DEFAULT_MAINTENANCE_BATCH = 128` 常量化**：消除 `default_batch_size()` 与
  `permissive_default()` 的魔法数字漂移。
5. `**TagConfig::default()` 去业务耦合**：core 层默认返回空 `seed_tags` / `deprecated_tags`，
  业务标签仅在 `DomainSchema.json` 中显式声明。
6. `**WikiPage` 新增 `entry_type: Option<EntryType>` 字段**：带
  `#[serde(default)]`，历史快照仍可无损反序列化；新增 `with_entry_type()` builder。
7. `**extract_headings(markdown)` 辅助函数**：从 Markdown 中提取 1~6 级 ATX heading 文本，
  用于完整度匹配。
8. `**check_page_completeness(schema, page) -> Vec<LintFinding>`**：
  当页面设置了 `entry_type` 且 schema 里该类型配置了必需段落时，扫描 heading 并
   产出 `page.incomplete` 的 `Warn` 级 finding。
9. `**LlmWikiEngine::run_basic_lint` 接入 completeness 检查**：在原有
  `page.broken_wikilink` 检查后，对每个可见页面追加 completeness findings。
10. `**DomainSchema::required_sections_for(&EntryType) -> &[String]`**：把条目类型到
  段落列表的路由收敛到 schema 上，avoiding 散落在调用点。
11. **注释清晰化 `required_sections` 两处职责**：
  - `PromotionConditions.required_sections` = 晋升到目标状态的**额外**段落；
    - `CompletenessConfig.*_required_sections` = **lint 基线**，无论是否晋升都检查。
    两者相互独立、不覆盖。

### 遇到了哪些错误

1. `**from_json_slice` 签名变更可能影响调用方**：之前是
  `Result<Self, serde_json::Error>`，改为 `Result<Self, SchemaLoadError>` 后，
   担心 CLI 或 kernel 的 `?` 传播链断掉。
2. 无编译错误（`cargo build --workspace` 一次通过）。

### 是如何解决这些错误的

1. 预先通过 grep 确认所有调用点都使用 `?` 传播或 `.expect()`，而 `SchemaLoadError`
  通过 `#[from]` 自动实现 `From<serde_json::Error>`、`From<SchemaValidationError>`，
   传播链天然兼容。

### 验证

- `cargo build --workspace`：5 个 crate 均干净编译。
- `cargo test --workspace`：**40 个测试全绿**
  - wiki-cli: 1
  - wiki-core: 23（原 10 + 新增 13：8 个 schema 校验、2 个 heading 提取、3 个完整度）
  - wiki-core schema_json: 3
  - wiki-kernel: 9
  - wiki-mempalace-bridge: 2
  - wiki-storage: 2
- `./scripts/e2e.sh`：**E2E PASS**，9 步全通过
（ingest → file-claim → supersede → query write-page → lint → outbox export/ack →
mempalace consume=4 → llm-smoke skip → viewer-scope 隔离验证）。

### 未完成项（后续 PR 跟进）

- CLI 层给 `query --write-page` / `file-page` 等命令加 `--entry-type` flag，
把 `WikiPage.entry_type` 真正用起来。
- `promote` 流程消费 `LifecycleRule.promotions`，配合 `min_age_days` /
`required_sections` / `cooldown_days` 完成自动晋升。
- ingest / auto_hooks 消费 `TagConfig`（`deprecated_tags` 拦截、
`max_new_tags_per_ingest` 限流）。
- `stale_days` / `auto_cleanup` 接入 maintenance 命令。

---

## 2026-04-21 · Phase 6a + 6b + entry-type flag

### 背景

合并 monorepo 后的架构清理。三个独立批次按「先框架后功能」的顺序推进：

- **批次 A (6a)**：wiki-cli 的 10 个 `mempalace`_* MCP 工具直连 `rust-mempalace`，
绕过了 `wiki-mempalace-bridge` 的抽象层。需要统一到 bridge。
- **批次 B (6b)**：`rust-mempalace` 使用 `edition = "2024"`，workspace 其余 crate 用
`edition = "2021"`。统一到 workspace edition 消除不一致。
- **批次 C**：CLI 产出的 `WikiPage` 缺少 `entry_type` 绑定，导致
`check_page_completeness` lint 无法触发。

### 实现了哪些功能

**批次 A — bridge 抽象化 mempalace 工具**

1. 新增 `crates/wiki-mempalace-bridge/src/tools.rs`：`MempalaceTools` trait
  （10 个方法，统一返回 `Result<Value, MempalaceError>`）+ `NoopMempalaceTools`
   空实现 + `make_tools()` 工厂函数。
2. 新增 `crates/wiki-mempalace-bridge/src/live_tools.rs`（`feature = "live"`）：
  `LiveMempalaceTools` 持有 palace 连接和配置，把原 wiki-cli mcp.rs 中
   `call_mempalace_tool` 的全部逻辑原样搬入，JSON 输出结构不变。
3. 重写 wiki-cli `call_mempalace_tool`：改为调 `wiki_mempalace_bridge::make_tools()`
  dispatch 到 trait 方法。
4. 删除 `crates/wiki-cli/Cargo.toml` 对 `rust-mempalace` 的直接 path 依赖。
  现在 wiki-cli 只认识 bridge，依赖链：`wiki-cli -> bridge -> rust-mempalace`。

**批次 B — edition 统一**

1. `crates/rust-mempalace/Cargo.toml` 的 `edition = "2024"` 改为 `edition.workspace = true`，
  同步加 `license.workspace = true`。
2. 编译零错误（无 2024-only 语法），仅 `cargo fmt` 产生 import 排序和 if-else 风格差异。

**批次 C — --entry-type flag**

1. `Cmd::IngestLlm` 和 `Cmd::Query` 新增 `--entry-type <VALUE>` 可选参数。
2. 新增 `parse_entry_type_opt()` helper：调用 `EntryType::parse()` strict 解析，
  不存在的值直接报错退出。
3. `IngestLlm` 的 summary page 和 `query_to_page` 都通过 `with_entry_type()` 绑定。
4. 新增 `crates/wiki-cli/tests/entry_type_flag.rs` 集成测试 3 个：
  - `query --write-page --entry-type concept` → lint 报 `page.incomplete`
  - `--entry-type nonexistent_type` → 报错退出
  - 不带 `--entry-type` → lint 不报 `page.incomplete`

### 遇到了哪些错误

1. `**Connection::path()` 返回 `Option<&str>` 而非 `Option<&Path>`**：
  `live_tools.rs` 的 `wake_up` 方法中，误用 `p.parent()` 导致编译失败。
   修复：先 `Path::new(p)` 再 `.parent()`。
2. **edition 2024→2021 的 rustfmt 差异**：import 分组排序、if-else 单行展开等。
  修复：`cargo fmt --all` 一次搞定。
3. **集成测试 binary 路径计算错误**：手动拼 `target/debug/wiki-cli` 路径不对。
  修复：使用 `CARGO_BIN_EXE_wiki-cli` 环境变量。
4. **EntryType 错误消息含中文"未知"**：测试断言写的是英文 `unknown`。
  修复：断言加上中文 `未知` 关键词。

### 是如何解决这些错误的

每批次独立 commit + push，8 轮分阶段测试确保问题早暴露。

### 验证

- `cargo fmt --all -- --check`：干净
- `cargo test --workspace`：**62 个测试全绿**
  - wiki-cli: 4（1 单测 + 3 新增集成测试）
  - wiki-core: 23 + schema_json: 3
  - wiki-kernel: 9
  - wiki-mempalace-bridge: 12（10 新增 Noop 工具单测 + 2 原有）
  - rust-mempalace + e2e_core: 9
  - wiki-storage: 2
- `scripts/e2e.sh`：**E2E PASS**
- MCP smoke：`tools/list` 返回 22 个工具不变，`mempalace_status` / `mempalace_kg_stats` 正常。

### 提交记录

1. `refactor(bridge): route wiki-cli mempalace_* tools through MempalaceTools trait`
2. `chore(rust-mempalace): unify edition to workspace (2021) + fmt`
3. `feat(cli): --entry-type flag on ingest-llm and query --write-page`（本批次）

---

## 2026-04-21 · Schema 后续项 T0 + T1 全闭环

### 背景

基于 `DomainSchema` 中已定义但未被消费的字段（`LifecycleRule`、`TagConfig`、`CompletenessConfig`），
按 T0/T1/T2/T3 四层分级路线图（见 `docs/schema-followup-plan.md`）实施 T0 + T1 全闭环。

设计裁决：

- `EntryStatus` 仅挂在 `WikiPage` 上，`Claim` 维持 `MemoryTier` + `stale: bool` 不动。
- `LifecycleRule.stale_days` 语义：`auto_cleanup = false` → 标记 `NeedsUpdate`；`auto_cleanup = true` → 删除。
- 向前兼容全靠 `#[serde(default)]`，不写数据迁移命令。

### 实现了哪些功能

**Phase 0 - 路线图落档**

- 新建 `docs/schema-followup-plan.md`：T0/T1/T2/T3 四层分级、mermaid 状态机图、设计裁决。

**T0 - 两项小改动**

1. `wiki schema validate [path]` 子命令：调用 `DomainSchema::from_json_path`，合法 exit 0 + 打印 summary，非法 exit 1 + 具体错误。
2. `--entry-type` 扩档到 Crystallize 链路：CLI `Cmd::Crystallize` 加 `--entry-type` 可选参数，MCP `wiki_crystallize` 工具加 `entry_type` 可选字段。

**T1 - Lifecycle 主线全闭环**

1. **T1.A**：`WikiPage` 新增 `status: EntryStatus`（serde default = Draft）+ `with_status` builder。
2. **T1.B**：`wiki-kernel` 新增 `initial_status_for(entry_type, schema)` 纯函数；CLI 三处 page 创建（IngestLlm/Query/Crystallize）+ MCP crystallize 全部接入。
3. **T1.C**：`promote_page(page_id, to_status, actor, now, force)` 方法 + `PromotePageError` 6 种错误变体 + CLI `Cmd::PromotePage` 子命令。`WikiEvent` 新增 `PageStatusChanged` 事件。
4. **T1.D**：`mark_stale_pages(now)` 方法 + `Cmd::Maintenance` 接入 + 输出 `pages_marked_needs_update=N`。
5. **T1.E**：`cleanup_expired_pages(now)` 方法 + `Cmd::Maintenance` 接入 + 输出 `pages_auto_cleaned=N`。`WikiEvent` 新增 `PageDeleted` 事件。

### 遇到了哪些错误

1. **Fixture JSON 使用 PascalCase enum 值**：`DomainSchema` 的 `EntityKind` 等使用 `#[serde(rename_all = "snake_case")]`，fixture 中 `Person` 应为 `person`。修复：所有 fixture 改为 snake_case。
2. `**--schema` 全局参数放在子命令之后**：clap 要求全局参数在子命令前。修复：调整测试中参数顺序。
3. `**page.entry_type` move 出 shared reference**：`Option<EntryType>` 不 impl Copy，`page.entry_type.ok_or(...)` 会 move。修复：加 `.clone()`。
4. `**WikiPage::new` 不自动解析 wikilinks**：测试中 ref_page 的 `outbound_page_titles` 为空。修复：手动调用 `refresh_outbound_links()`。
5. `**println!` 多余的 `);`**：替换 Maintenance 输出时引入重复分号。修复：删除多余 `);`。

### 提交记录

1. `feat(T0): schema validate 子命令 + crystallize entry_type 扩档 + 路线图文档`
2. `feat(T1): WikiPage lifecycle 全闭环 — status/promote/stale/cleanup`（本批次）

---

## D1–D4 Dogfood Readiness（2026-04-21）

### 实现了哪些功能

**D1 — Projection YAML frontmatter**

- `crates/wiki-kernel/src/wiki_writer.rs` 新增三类渲染函数与辅助函数：
  - `yaml_escape(s)` — 双引号 / 反斜杠转义
  - `scope_label(scope)` — Scope → "private:xxx" 字符串
  - `status_str(s)` / `entry_type_str(t)` — enum → snake_case（避免引入 serde_json）
  - `render_page_with_frontmatter(page)` — pages/*.md 加 id/title/status/entry_type/scope/updated_at
  - `render_claim_with_frontmatter(claim)` — concepts/*.md 加 id/tier/confidence/quality/stale/sources_count/created_at
  - `render_source_with_frontmatter(source)` — sources/*.md 加 id/uri/ingested_at
- `write_projection` 的三处 `fs::write` 全部替换为上述渲染函数
- 新增 8 个单元测试（7 个 frontmatter 行为 + 原有 1 个增强为集成断言）
- `scripts/e2e.sh` 第 [5.1] 步新增 frontmatter 完整性检查

### 遇到了哪些错误

1. **`serde_json` 不在 wiki-kernel 依赖**：最初用 `serde_json::to_value` 序列化 `EntryStatus` 和 `EntryType`，编译失败。改用手写 `status_str` / `entry_type_str` match 函数解决，无需新增依赖。
2. **`cargo fmt` 格式差异**：import 行被 fmt 折叠为单行，render_page 中两个 `format!` 调用被合并。执行 `cargo fmt --all` 修复。

### 如何解决

- 改用 no-dep 的 match 辅助函数代替 serde_json 序列化，保持 wiki-kernel 最小依赖。
- 所有格式问题由 `cargo fmt --all` 统一处理。

---

## D2 — needs_update → approved 反向 promotion（2026-04-21）

### 实现了哪些功能

- `DomainSchema.json`：concept/entity 与 synthesis 两条 lifecycle_rule 各自新增一条反向 promotion：
  `needs_update → approved`，条件全部置 0（min_age_days=0 / required_sections=[] / min_references=0 / cooldown_days=null）。
- `crates/wiki-kernel/src/engine.rs` 新增两条单元测试：
  - `promote_needs_update_to_approved_works` — 反向规则存在时，emit `PageStatusChanged(NeedsUpdate → Approved)`
  - `promote_needs_update_without_rule_still_errors` — 无规则时仍返回 `NoPromotion` 回归保护
- 新增 `crates/wiki-cli/tests/fixtures/schema_with_reverse_promotion.json`（无环设计：draft→needs_update→approved）
- 新增集成测试 `crates/wiki-cli/tests/promote_page_recover_from_stale.rs`：
  crystallize(concept) → promote --force --to needs_update → promote --to approved（走反向规则，无 force）

### 遇到了哪些错误

1. **`schema invalid: PromotionCycle`**：最初 fixture 同时含 `approved→needs_update` 和 `needs_update→approved` 形成环。Schema 加载期环检测拒绝。
2. **`ParseChar index=37`**：`parse_page_id` 将整行 `page=<uuid> claims=N` 传入 UUID 解析，空格导致失败。
3. **`repo_domain_schema_lifecycle_rules_indexable` 断言**：DomainSchema 中 concept promotions 从 2 变 3（加了反向规则），旧断言失败。

### 如何解决

1. 把 fixture 重新设计为 `draft→needs_update→approved` 线性路径（不含反向环），既能覆盖反向规则又满足环检测；真实 `DomainSchema.json` 的 `approved→needs_update` 由 `mark_stale_pages` 直接 mutate 而非 promotion，因此无冲突。
2. 修改 `parse_page_id`：先 `split_whitespace().next()` 再解析，稳健处理 CLI 多字段输出格式。
3. 更新 `crates/wiki-core/tests/schema_json.rs` 的 `promotions.len()` 期望值为 3，并补充注释说明三条规则含义。

### 验证

- `cargo run -p wiki-cli -- schema-validate DomainSchema.json` → `schema ok: lifecycle_rules=6`
- `cargo test --workspace` → 全绿（本次改动涉及的相关 suite 全部通过）
---

## D3 — ingest-llm 默认 entry_type + MCP 对齐（2026-04-21）

### 实现了哪些功能

- `crates/wiki-cli/src/main.rs` 新增 `effective_ingest_entry_type(Option<EntryType>) -> EntryType`：未显式指定时回退 `EntryType::Concept`。
- `Cmd::IngestLlm` summary 页面生成改用新 helper：无论 LLM / 用户是否传 `--entry-type`，都会产出带 `entry_type=concept`（或显式值）的页面并正确计算 `initial_status_for`。
- `crates/wiki-cli/src/mcp.rs` 的 `wiki_ingest_llm` 工具：
  - `inputSchema.properties` 新增 `entry_type`（string，description 提示默认 concept）；
  - handler 解析 `entry_type` 参数、调用 `parse_entry_type_opt` 做严格校验、按 `effective_ingest_entry_type` 回退；
  - 若 `plan.summary_markdown` 非空，补齐 summary page 写入（此前 MCP 路径只落 claims / source，不落 page，与 CLI 行为不一致，现在对齐）；
  - 向量开关打开时对 claim 做 best-effort 嵌入（对齐 CLI 行为）；
  - 返回体新增 `summary_page_id`。
- 单元测试 `mcp::tests::tools_list_wiki_ingest_llm_has_entry_type_param`：断言 `tools/list` 暴露的 schema 含 `entry_type:string` 且描述含 `concept`。
- 单元测试 `tests::effective_entry_type_defaults_to_concept` / `tests::effective_entry_type_preserves_explicit`。

### 遇到了哪些错误

- 无编译 / 测试失败；一次 `_ = viewer` 赘余赋值被及时删除。

### 如何解决

- 将所有缺省策略集中在 `effective_ingest_entry_type` 单点实现，CLI/MCP 双路径共用。
- MCP handler 通过 `eng.schema.clone()` 取到 DomainSchema，复用 `initial_status_for` 保持与 CLI 一致的生命周期初始状态。

### 验证

- `cargo fmt --all --check`（隐式 via `cargo fmt --all` 后重跑）
- `cargo test --workspace`：全绿（所有 test result 均 ok）
---

## D4 — SQLite 热备份脚本（2026-04-21）

### 实现了哪些功能

- 新增 `scripts/backup.sh`：
  - 使用 `sqlite3 .backup` 在线热备（兼容 WAL，不阻塞业务写入）；
  - 参数 `--db / --wiki / --out`，默认 `~/wiki-mempalace/{wiki.db,wiki,backups}`；
  - 备份后跑 `PRAGMA integrity_check` 校验；
  - 若 wiki 投影目录存在，额外打包 `wiki-<ts>.tar.gz`；
  - 输出 `BACKUP_DB=<path>` 便于脚本链式消费。
- `scripts/e2e.sh` 新增 `[10] backup smoke`：调用脚本、校验备份库可打开并含 `wiki_state` / `wiki_outbox` 表、tar.gz 存在。

### 遇到了哪些错误

1. 首次 smoke 用 `pages` 作为存在性标志，但 SQLite schema 实际是 `wiki_state / wiki_outbox`（page / claim 作为 JSON 存在 `wiki_state.data` 中），导致断言失败。

### 如何解决

- 把断言改为 `wiki_state` + `wiki_outbox` 两张核心表的存在性；与存储层实际 schema 对齐。

### 验证

- `bash scripts/e2e.sh` → `E2E PASS`，`[10] backup smoke OK` 日志正常。

---

## 2026-04-22 · Notion → 本地 Wiki 全量迁移

### 背景

三个 Notion 数据库（📚 知识 Wiki 3372 条、🐦 X书签 674 条、微信 420 条）长期作为知识管理载体，
但 Notion 的链接机制和检索能力不够灵活，需要迁移到本地 Obsidian vault 以配合 `wiki-mempalace` 系统使用。

### 实现了哪些功能

1. **`wiki-migration-notion` crate**（新增，位于 `llm-wiki/crates/wiki-migration-notion/`）：
   - `scanner`：递归扫描 Notion Export 解压目录，过滤 `.md` 文件
   - `parser`：解析 Notion Markdown 格式（UUID 文件名提取 + `Key: value` 属性块 + 正文链接抽取）
   - `resolver`：构建 UUID 索引和 URL 索引，解析内部边和外部边（Wiki→Source）
   - `report`：生成 `migration-report.md` 干跑报告
   - `writer`：落盘到本地 vault 目录，按 `entry_type` 分子目录，YAML frontmatter + 链接改写
   - CLI 子命令 `dry-run` / `migrate`

2. **三库全量迁移执行**：
   - 4477 条 Markdown（Wiki 3377 + X书签 674 + 微信 426）
   - 12804 条内部边（99.6% 解析）
   - 4313 条外部边（Wiki→Source，URL 命中 3699 + 源文章URL 字段 614）
   - 1072 条伪 URL 清洗（`claude.md` 等 Notion 自动链接化 bug）
   - 266 条孤儿 source 标记
   - 0 数据丢失，0 文件名碰撞

3. **Obsidian vault 落盘**：产物从 `/tmp/wiki-migrated/` 搬家到 `~/Documents/wiki/`，用户 Obsidian 验证通过

4. **`dogfood-readiness.md` 大更新**：U1–U5 + D1–D4 全标 ✅，附录 B 改为实际目录结构，附录 C 改为已完成迁移记录

### 遇到了哪些错误

1. **Notion Export ZIP 为空（22 字节）**：用户首次下载时尚未等待 Notion 完成导出。重新等待邮件通知后获取正确 ZIP。
2. **macOS `unzip` UTF-8 文件名失败**（Illegal byte sequence）：中文文件名导致系统自带 unzip 崩溃。改用 `ditto -x -k` 原生解压。
3. **slugify 单测失败**：全角冒号 `：` 在 macOS 上是合法文件名字符，但测试期望被替换。修正测试断言。
4. **YAML frontmatter 布尔值被引号包裹**：`compiled_to_wiki: "true"` 应为 `compiled_to_wiki: true`。新增 `fm_raw()` 辅助函数直接输出。
5. **文件名碰撞解决不够健壮**：初始只用 uuid8 后缀，改为三级回退（base → base-uuid8 → base-fulluuid）。
6. **macOS Unicode NFD/NFC 归一化幻觉**：Python 脚本报告"缺少文件"，实际是文件系统 NFD 与字符串 NFC 比较差异。逐文件 `os.path.exists()` 确认 0 缺失。

### 是如何解决这些错误的

1. 指导用户正确操作 Notion Export 流程。
2. 使用 macOS 原生 `ditto` 命令替代 `unzip`。
3. 修正测试断言，匹配 macOS 实际行为。
4. 区分 `fm()`（带 YAML 转义）和 `fm_raw()`（原始输出）。
5. 三级文件名回退策略。
6. 诊断 Unicode 归一化差异，确认全部文件物理存在。

### 验证

- `cargo test -p wiki-migration-notion`：7 个单测全绿
- `cargo fmt --all --check`：干净
- dry-run 报告：4477 条解析成功，统计数字与 Notion 原始数据吻合
- migrate 落盘后文件数验证：4477 个 `.md` 文件全部存在
- Obsidian 打开 vault：frontmatter 正确、内部链接可点击、搜索正常
- Git commit：`2d0e9d8`（llm-wiki 仓库）

---

## 2026-04-22 · 孤儿 Source 审计

### 背景

迁移完成后，266 条 source（256 微信 + 10 X书签）被标记为 `orphan: true`，即没有任何 Wiki 页面通过链接引用它们。其中 257 条在 Notion 里标了 `已编译到Wiki=Yes`。需要审计根因并分类处理。

### 实现了哪些功能

1. **`wiki-migration-notion` 新增 `audit-orphans` 子命令**：
   - `audit.rs` 模块：扫描 vault 目录提取孤儿元数据、Wiki 页正文、标题模糊匹配、A/B/C 分类
   - 标题匹配算法：生成多个搜索模式（完整标题 / 去标点核心子串 / 前 15 字符），在 3376 个 Wiki 页正文中搜索
   - 分三类：A（标题匹配到→需补链接）、B（已编译未匹配→疑似标记错误）、C（未编译→正常孤儿）
   - 输出 Markdown 报告 + 结构化 JSON

### 审计结果

| 分类 | 数量 | 含义 |
| --- | --- | --- |
| A. 标题匹配到 | **173** | Wiki 正文确实提到了，但用纯文本无链接 |
| B. 已编译未匹配 | **84** | Notion 标记已编译但找不到引用 |
| C. 未编译 | **9** | 从未编译，正常孤儿 |

**A 类细分**：
- 117 条在 concept/entity/synthesis 页面中被引用 → 真正需要补链接
- 56 条仅在 summary/lint-report 中匹配 → summary 本身是 source 的编译结果，关系已隐式存在
- 161 条 summary 匹配主要是 H1 标题包含（`摘要：{source标题}`），属于"自引用"

**微信是重灾区**：256/266（96%），X 仅 10 条。微信文章多被纯文本 `《标题》` 引用，无 URL 可匹配。

### 遇到了哪些错误

1. **UTF-8 字符边界 panic**：截取上下文时用字节偏移 `body[start..end]`，中文字符占 3 字节，切片落在字符中间。修复为 `.chars().skip().take()` 确保字符边界安全。

### 是如何解决这些错误的

1. 将字节偏移改为字符偏移：`body.chars().skip(char_start).take(char_end - char_start).collect()`。

### 验证

- `cargo test -p wiki-migration-notion`：19 个单测全绿（含 4 个新增 audit 测试）
- 审计报告 + JSON 已落盘到 `~/Documents/wiki/.wiki/orphan-audit-report.md`

---

## 2026-04-22 · A 类孤儿自动补链接

### 背景

审计发现 173 条 A 类孤儿（标题在 Wiki 页正文中被纯文本引用但无链接），需要在匹配到的 Wiki 页面中自动插入指向 source 的 Markdown 链接。

### 实现了哪些功能

1. **`wiki-migration-notion` 新增 `fix-orphans` 子命令**：
   - 读取审计 JSON，对 A 类孤儿在 Wiki 页正文中找到标题出现行
   - 使用最长匹配模式（避免子串误匹配）
   - 在行尾追加 `（[source](相对路径)）` 格式的链接
   - 跳过已有 summary/source 链接的页面
   - 按文件分组、从后往前插入（避免字节偏移漂移）

### 执行结果

- **72 个 Wiki 页面被修改**
- **88 条 source 链接被插入**
- **323 条匹配被跳过**（已有链接）
- **0 条位置错误**（全部插入在行尾）

### 遇到了哪些错误

1. **链接插入在标题文字中间**（首版）：`find` 匹配到标题子串后直接在匹配位置插入，导致 `《110K Stars 背后的共（[source](...)）识：...》`。修复：改用最长匹配 + 在行尾插入。
2. **UTF-8 字符边界问题**（同审计阶段）：同上，改用字符级操作。

### 是如何解决这些错误的

1. 先回退 72 个被改坏的文件（用 Python 正则删除所有 `（[source](...)）`），然后用修正后的代码重新运行。
2. 行尾插入策略：`find_line_end()` 找到匹配位置所在行的 `\n`，在换行符前插入。

### 验证

- Python 脚本逐行验证 88 条链接全部在行尾，0 条位置错误
- 手动检查 `规范驱动开发-SDD.md` 等示例页面，链接格式正确、相对路径可用
- Obsidian 中点击链接可跳转到对应 source 页面

---

## 2026-04-22 · B2+C 类统一标记为未编译

### 背景

B1 归一化匹配后发现全部 6 条候选都已有 summary 链接（关系链完整），无需补链接。
剩余 B2（78 条）和 C 类（9 条）无任何引用关系，统一归入"未编译队列"。

### 实现了哪些功能

- 将 78 条 B2 孤儿的 `compiled_to_wiki` 从 `true` 改为 `false`
- C 类 9 条已经是 `false`，无需修改

### 最终孤儿分布

| 状态 | 数量 | 含义 |
| --- | --- | --- |
| compiled=true | 179 | 有 summary 页面作为桥梁，关系链完整（A 类 + B1） |
| compiled=false | 87 | 未编译队列（B2 的 78 + C 的 9），留给后续 LLM 编译 |
| **合计** | **266** | |

---

## 2026-04-22 · `batch-ingest`：未编译 source 批处理 LLM 编译

### 背景

B2+C 类统一为 `compiled_to_wiki: false` 后，约 **78** 条有正文、**1** 条空正文
（扫描时按正文过短跳过）。需要一条可重复运行的 CLI 命令：在 Obsidian vault
上发现待编译条、逐条走与 `ingest-llm` 等价的落库与（可选）投影，成功后在
source 文件 frontmatter 中写回 `compiled_to_wiki: true`，并支持 dry-run、
条数限制与请求间隔，避免只依赖手工逐条调用。

### 实现了哪些功能

1. **`wiki-cli` 新增子命令 `batch-ingest`**
   - 参数：`--vault`（含 `sources/` 的根目录）、`--limit`、`--dry-run`、
     `--entry-type`（解析校验，与 summary 侧默认策略一致时可用）、`--delay-secs`。
   - 扫描 `vault/sources/**/*.md`，frontmatter 中 `compiled_to_wiki: false` 且正文
     长度 ≥ 50 字节的文件进入队列，按标题排序后处理。
2. **抽取 `ingest_one_source`**：单条与 `IngestLlm` 同一管线（`complete_chat` →
   `LlmIngestPlanV1` → `ingest_raw`、claims、entities、edges、summary page），
   批处理结束后可选 `maybe_sync_projection`。
3. **依赖**：`crates/wiki-cli` 增加 `walkdir`、`regex`。
4. **`wiki-core` 解析增强**
   - `LlmIngestPlanV1.claims` 使用自定义反序列化：同时接受 `[{ "text", "tier" }]` 与
     `["纯字符串", …]`，后者自动补 `tier: semantic`。
5. **健壮性**
   - `parse_memory_tier` 失败时回退为 `MemoryTier::Semantic`（例如模型拼写
     `sematic`）。
   - `add_entity` / `add_edge` 若被 schema 拒绝则跳过该条，不使整篇失败
     （`ingest-llm` 与批处理一致）。
6. **数据侧结果**（在 `~/Documents/wiki` 上跑通）
   - 有正文的条目共编译完成；`rg` 仅剩 **1** 条 `compiled_to_wiki: false`（无标题空正文）。

### 遇到了哪些错误

1. **工作区路径混淆**：曾误改 `llm-wiki/crates/wiki-cli`，而实际构建的是
   `wiki-mempalace` 仓内同路径；子命令在错误目录下不可见。统一到本仓实现。
2. **LLM 输出 `claims` 为字符串数组**：与 `LlmClaimDraft` 结构不匹配，整篇 parse 失败。
3. **LLM 将 `semantic` 拼成 `sematic` 等**：`parse_memory_tier` 导致整条 ingest 失败。
4. **Schema 拒绝部分 entity `kind` / relation**：`add_entity?` 直接返回错误，整篇被计为失败。
5. **全量首跑**：约 21 条因（4）失败；全量总览约 **成功 53 / 失败 21**（终端日志 `944655`）。
6. **上游 API 偶发返回 `message.content: null`**：单条重跑 `batch-ingest` 后成功。

### 是如何解决这些错误的

1. 在 **本仓库** `wiki-mempalace/crates/wiki-cli` 与 `wiki-core` 中落地实现，不在
   旁路旧路径修改。
2. 在 `llm_ingest_plan.rs` 为 `claims` 增加 `deserialize_claims_flexible`（对象 / 字符串
   混排），并补单元测试。
3. `ingest-llm` 与 `ingest_one_source` 内对 tier 使用 `unwrap_or(MemoryTier::Semantic)`。
4. 将 `add_entity` / `add_edge` 改为忽略 schema 单条拒绝（`let _ = …`），保留 claims
   与 source 入库。
5. 对剩余失败条目在修复代码后**再次全量/多次**运行 `batch-ingest`，直到队列清空。
6. 对 `content: null` 的条目**单独再跑一次**同命令，通常第二次模型返回有效 JSON。

### 验证

- `cargo test -p wiki-core`：`llm_ingest_plan` 相关用例通过（含字符串 `claims` 用例）。
- `cargo run -p wiki-cli -- batch-ingest --vault ~/Documents/wiki --dry-run`：条数与预期一致
  （有正文条数 = 待处理数）。
- `cargo run -p wiki-cli -- batch-ingest --vault ... --limit 1|2`：小样本端到端通过。
- 收尾后：`rg` 在 `~/Documents/wiki/sources/` 下 `compiled_to_wiki: false` 仅剩 1
  条空正文；其余已成功写库并回写 `true`。
- Git 提交 `45743d4`：feat(cli) 批处理与解析增强的代码部分。

### 相关文档与 Git

- 本次将 `README.md`（`batch-ingest` 使用示例）与 `docs/plan.md`（M6 与后续列表）
  与本条 `Progress` 同步提交。

