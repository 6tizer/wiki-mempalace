# llm-wiki 工作进度

按用户规范逐步记录每一次有意义的工作。每条记录包括：实现的功能 / 遇到的错误 / 解决方式。

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
2. **`crates/wiki-core/src/lib.rs` 补齐 re-export**：新增
   `CompletenessConfig / EntryStatus / EntryType / LifecycleRule / PromotionConditions /
   PromotionRule / TagConfig` 的 public re-export，让下游 crate 和 CLI 能够引用这些类型。
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
   根因：`model.rs` 的 `EntityKind / RelationKind / MemoryTier` 与新增的 `EntryType /
   EntryStatus` 都使用 `#[serde(rename_all = "snake_case")]`，但 JSON 中仍是 PascalCase。
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

1. **`EntryType::parse` / `EntryStatus::parse` 改为 `Result<Self, SchemaValidationError>`**：
   未知输入不再静默回落到 `Draft` / `Concept`，避免拼写错误被静默吃掉。
2. **`SchemaValidationError` 与 `SchemaLoadError::Invalid(..)`**：新增语义错误枚举，
   覆盖重复 EntryType / promotion 自环 / 环路 / 未知字面量 4 种情况。
3. **`DomainSchema::validate()`**：
   - 规则 1：任一 `EntryType` 最多出现在一条 `LifecycleRule.entry_types` 中；
   - 规则 2：任一 `PromotionRule` 的 `from_status != to_status`；
   - 规则 3：每条 rule 的 promotions 形成的有向图无环（三色标记 DFS）。
   `from_json_slice` / `from_json_path` 在返回前自动调用 `validate`，bad schema 快速失败。
4. **`DEFAULT_MAINTENANCE_BATCH = 128` 常量化**：消除 `default_batch_size()` 与
   `permissive_default()` 的魔法数字漂移。
5. **`TagConfig::default()` 去业务耦合**：core 层默认返回空 `seed_tags` / `deprecated_tags`，
   业务标签仅在 `DomainSchema.json` 中显式声明。
6. **`WikiPage` 新增 `entry_type: Option<EntryType>` 字段**：带
   `#[serde(default)]`，历史快照仍可无损反序列化；新增 `with_entry_type()` builder。
7. **`extract_headings(markdown)` 辅助函数**：从 Markdown 中提取 1~6 级 ATX heading 文本，
   用于完整度匹配。
8. **`check_page_completeness(schema, page) -> Vec<LintFinding>`**：
   当页面设置了 `entry_type` 且 schema 里该类型配置了必需段落时，扫描 heading 并
   产出 `page.incomplete` 的 `Warn` 级 finding。
9. **`LlmWikiEngine::run_basic_lint` 接入 completeness 检查**：在原有
   `page.broken_wikilink` 检查后，对每个可见页面追加 completeness findings。
10. **`DomainSchema::required_sections_for(&EntryType) -> &[String]`**：把条目类型到
    段落列表的路由收敛到 schema 上，avoiding 散落在调用点。
11. **注释清晰化 `required_sections` 两处职责**：
    - `PromotionConditions.required_sections` = 晋升到目标状态的**额外**段落；
    - `CompletenessConfig.*_required_sections` = **lint 基线**，无论是否晋升都检查。
    两者相互独立、不覆盖。

### 遇到了哪些错误

1. **`from_json_slice` 签名变更可能影响调用方**：之前是
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
