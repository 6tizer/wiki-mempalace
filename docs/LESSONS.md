# Lessons

本文记录每轮开发后的项目级经验。新对话进入 Plan mode 前必须先读本文件，再读当前 PRD 和 spec。

## 记录格式

每次合并后追加一节：

```markdown
## <date> / <PR or module>

- Scope:
- What worked:
- What caused rework:
- Spec changes needed:
- Tests or reviews that caught issues:
- Next plan note:
```

## Current Notes

- 大模块先拆 PRD，再拆 spec 三件套。不要直接从 issue list 写代码。
- spec 和代码冲突时，先修 spec，再修代码。PRD 范围变化必须让用户决定。
- subagent 任务要有 owner files，避免并行写同一文件。
- 每个模块完成后写 handoff，比把完整对话历史带到下一轮更稳。
- Agent-facing CLI 默认值不要依赖 cwd；只要语义属于 vault 输出，相对路径应在
  `--wiki-dir` 存在时解析为 vault-relative，并用测试固定。

## 2026-04-30 / AI Provider Profiles

- Scope: PR1 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3; task-level LLM profiles and Exa/Tavily web search evidence runtime.
- What worked: Keeping `[llm]` as default preserved existing ingest/query/MCP paths while future Fixer/Synthesis can opt into named profiles.
- What caused rework: Parallel `cargo test` commands only created Cargo lock contention; use `cargo test -p wiki-cli --bin wiki-cli <filter>` for focused unit checks.
- Spec changes needed: Future Fixer/Synthesis specs should reference profile names instead of embedding provider/model choices in feature logic.
- Tests or reviews that caught issues: Focused LLM/web_search tests caught config behavior; workspace test/clippy/deny stayed green.
- Next plan note: Continue with `codex/wiki-governance-scan` after PR1 merge.

## 2026-04-30 / Wiki Governance Scan

- Scope: PR2 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3; read-only governance scan for lifecycle, references, lint, gaps, duplicates, retire candidates, and synthesis tag signals.
- What worked: Keeping the scanner pure over `InMemoryStore` avoided outbox/projection side effects and made the JSON report usable as PR3 Fixer input.
- What caused rework: `eng.run_basic_lint` is tempting but mutates audit/outbox state; governance scans should call pure `collect_basic_lint_findings` instead.
- Spec changes needed: Future Fixer specs should consume the typed scan JSON, not scrape Markdown reports.
- Tests or reviews that caught issues: Scope-filtered kernel tests fixed duplicate/reference/synthesis signal behavior before CLI wiring.
- Next plan note: PR3 can build typed fixer plans from `governance scan` JSON and only add LLM/search where evidence rules need them.

## 2026-04-30 / Evidence Fixer Plan

- Scope: PR3 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3; typed dry-run fixer plans from governance scan JSON.
- What worked: Handling `fixer-plan` before runtime open gives a hard read-only boundary and lets tests prove the command does not create a DB.
- Pitfall avoided: Near-duplicate merge cannot be made executable from fuzzy local similarity alone; it stays blocked unless the web verification key crosses the provider/domain threshold.
- Test gate: focused `wiki-core evidence_fixer`, `wiki-kernel evidence_fixer_plan`, `wiki-cli --test governance_fixer_plan`, workspace fmt/test/clippy, and `cargo deny` all pass locally before PR.
- Next plan note: PR4 should consume this typed plan, recheck current state before each apply, and write tombstones before merge/retire/semantic patch actions.

## 2026-04-30 / Evidence Fixer Apply + Restore

- Scope: PR4 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3; evidence-auto apply, tombstones, and restore.
- What worked: Keeping tombstones as typed JSON artifacts makes retire/semantic patch reversible without direct Palace writes.
- Pitfall avoided: Apply must recheck the live DB before mutation; missing subjects are skipped, and semantic patch line ranges must still match old text.
- Test gate: focused `wiki-core evidence_fixer`, `wiki-kernel evidence_fixer_apply`, `wiki-cli --test governance_fixer_apply`, `wiki-cli --bin wiki-cli governance`, workspace fmt/test/clippy, and `cargo deny` all pass locally before PR.
- Next plan note: PR5 should consume governance scan synthesis signals and avoid duplicate candidate topics.

## 2026-04-30 / Synthesis Discovery

- Scope: PR5 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3; point/line/plane/body candidate discovery from governance scan signals.
- What worked: Reusing `GovernanceScanReport` kept discovery read-only and made `--scan` no-engine mode easy to prove in CLI tests.
- Pitfall avoided: Three-tag synthesis is a pairwise triangle rule, not a triple-intersection rule; tests must pin that difference.
- Spec changes needed: Quad discovery needs existing synthesis topic coverage in the scan so it can avoid proposing body-level jumps without anchors.
- Tests or reviews that caught issues: Clippy caught `field_reassign_with_default`; focused review replaced naive triple loops with pair-graph triangle enumeration.
- Next plan note: PR6 should consume discovery candidates, build internal evidence packs, run dual-provider web research, then write `in_review/high` synthesis pages only after verifier checks.

## 2026-04-30 / Web-backed Synthesis Composer

- Scope: PR6 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3; internal evidence pack, dual web evidence, LLM draft, verifier, and guarded synthesis page write.
- What worked: Keeping fake `--web-evidence`, `--draft-json`, and `--verifier-json` inputs on the same compose path made the web/LLM chain testable without live provider calls.
- Pitfall avoided: Private-scope web search must block before external query generation unless `--allow-private-web-search` is explicit.
- Spec changes needed: Automation docs should treat blocked compose reports as first-class artifacts, not failures to ignore.
- Tests or reviews that caught issues: CLI compose tests caught the DB write path and private-web block; clippy caught needless generic borrows in section rendering.
- Next plan note: PR7 should wire daily/manual jobs and MCP/docs around `governance scan`, Fixer, discovery, and synthesis run without changing DB-first write semantics.

## 2026-04-30 / Governance Automation Docs

- Scope: PR7 of Wiki Governance + Evidence Fixer + Multi-provider Synthesis v3; automation jobs, daily/manual lanes, MCP boundary, Notion workflow matrix, and docs closeout.
- What worked: Reusing the existing automation registry kept job status, heartbeat, writer lease, and health reporting in one place.
- Design decision: `lint` stays available as a manual job, but daily now uses `governance-scan` plus `maintenance` to avoid a duplicate standalone lint step.
- Safety boundary: Governance/Fixer/Synthesis batch apply stays CLI-only; MCP remains live tool access and does not expose unattended `fixer-apply` or `synthesis-run`.
- Tests or reviews that caught issues: focused automation tests caught the changed `lint daily=no` contract after daily lane reordering.
- Next plan note: v3 is closed after PR7 merge; next work should come from a new PRD/spec unless production operation asks for a dry-run/apply cycle.

## 2026-04-30 / Audit Disposition PR 1 MCP Query Storage Ports

- Scope: MCP `wiki_query` default path now follows storage-backed query behavior instead of in-memory-only search.
- What worked: Reusing `SqliteSearchPorts` kept MCP behavior aligned with CLI query/explain without changing the MCP result shape.
- What caused rework: `wiki_query` records `QueryServed` and saves the engine snapshot, so stale memory can erase persisted rows unless the engine is reloaded before query.
- Spec changes needed: Future query-path PRs must state whether the path is read-only or records query events, because persistence side effects affect safe fallback design.
- Tests or reviews that caught issues: Focused MCP tests cover persisted-only retrieval, invalid palace fallback, query hash outbox, and write-page projection compatibility.
- Next plan note: After PR1 merge, continue with docs consistency cleanup for edition and Notion sync state.

## 2026-04-30 / Audit Disposition PR 2 Doc Consistency

- Scope: Active docs now describe workspace edition 2021 inheritance and completed Notion sync state without changing archived history.
- What worked: Using `rg` over active docs found the real contradiction and separated it from archive/spec historical context.
- What caused rework: Acceptance wording can itself contain stale phrases, so docs checks should avoid quoting the obsolete claim verbatim in active docs.
- Spec changes needed: Docs consistency specs should define active-doc search boundaries and archive handling.
- Tests or reviews that caught issues: Focused docs review plus active-doc `rg` checks caught edition 2024 wording and Notion sync stale-phrase echoes.
- Next plan note: After PR2 merge, refresh MCP API reference and add docs sync tests.

## 2026-04-30 / Audit Disposition PR 3 MCP API Reference

- Scope: MCP API reference now tracks current unified `wiki-cli mcp` tools, typed errors, scope rules, side effects, and mempalace bank capability.
- What worked: A docs sync test tied the reference to `tools_list()`, so future tool additions cannot silently skip docs.
- What caused rework: Mempalace `bank_id` must still be mentioned as a rejected field, but never in tool client arg columns.
- Spec changes needed: API docs specs should separate unified `wiki-cli mcp` from standalone `rust-mempalace mcp`.
- Tests or reviews that caught issues: Focused docs tests cover all tool names, error kinds, and mempalace bank arg table columns.
- Next plan note: 2026-04-30 disposition batch is closed; next work should start from a fresh PRD/spec if user selects a new target.

## 2026-04-29 / Audit v2 PR 01 MCP Input Boundary

- Scope: MCP result limit clamp + lint report path guard，先处理审计 P0 quick fix。
- What worked: schema bound、handler clamp、checked conversion 同时落；MCP 和 standalone mempalace 各自加 focused tests。
- What caused rework: reports 目录 symlink guard 不够，文件级 symlink 仍会被 `fs::write` 跟随。
- Spec changes needed: path guard 验收必须覆盖 directory symlink 和 file symlink 两层。
- Tests or reviews that caught issues: security-focused review 抓到 file-level symlink overwrite；新增回归测试后全 workspace gate 通过。
- Next plan note: 下一 PR 做 MCP scope/capability hardening，不混入 bank derivation。

## 2026-04-29 / Audit v2 PR 02 MCP Scope Capability

- Scope: MCP 写工具 scope capability 和 supersede old-claim viewer gate。
- What worked: 把能力边界收在 MCP resolver，缺省仍用 server viewer，显式 scope 必须 exact match。
- What caused rework: 旧测试明确断言 client 可覆盖 scope；先翻转测试再改实现更清楚。
- Spec changes needed: 本 PR 不做 capability token / sub-scope delegation，后续需要新 spec。
- Tests or reviews that caught issues: security review 抓到 maintenance 全库 decay 和 query/crystallize 显式 scope mismatch；handler 级测试固定 cross-scope 写入不变更状态，hidden old claim supersede 不生成新 claim。
- Next plan note: 下一 PR 做 mempalace bank capability，禁止 client 任意传 `bank_id`。

## 2026-04-29 / Audit v2 PR 03 Mempalace Bank Capability

- Scope: wiki MCP mempalace bank capability 和 KG bank 隔离。
- What worked: 在 wiki MCP 边界拒绝 client `bank_id`，再把 server viewer scope 映射成 palace bank，避免 trusted service API 和 untrusted client input 混在一起。
- What caused rework: KG 和显式 tunnel 原表没有 `bank_id`，只过滤 drawers 不够；必须迁移 `kg_facts` / `tunnels` 并同步 bridge live sink / graph ports。
- Spec changes needed: standalone `rust-mempalace` MCP 没有 wiki viewer scope，因此保留 local `bank_id` filter，不纳入本 PR 的 capability 边界。
- Tests or reviews that caught issues: focused tests 覆盖 MCP schema/拒绝 client bank，以及 KG query/timeline/stats/invalidate bank-scoped 行为；security review 抓到显式 tunnel traverse/status 泄漏和旧 schema bank index migration 顺序问题。
- Next plan note: 下一 PR 做 cross-bank drawer dedupe，把 `drawers(content_hash)` 从全局唯一迁到 `(bank_id, content_hash)`。

## 2026-04-29 / Audit v2 PR 04 Cross-Bank Drawer Dedupe

- Scope: `drawers(content_hash)` 全局唯一迁移为 `(bank_id, content_hash)`。
- What worked: 先改 SQLite 唯一索引，再同步 mine/live sink 的 preflight 查重，保证 DB 约束和应用层跳过逻辑一致。
- What caused rework: 无。
- Spec changes needed: PR4 不改 `content_hash` 计算；同一 source/content 在不同 bank 共享 hash，但唯一性由 bank 维度区分。
- Tests or reviews that caught issues: 旧 schema migration 测试覆盖 legacy index drop + composite index；mine/live sink 测试覆盖跨 bank 可共存、同 bank idempotent；migration review 抓到 `mine_path_convos` 覆盖缺口。
- Next plan note: 下一 PR 做 QueryServed privacy/schema，避免 query 原文进入 outbox/history。

## 2026-04-29 / Audit v2 PR 05 QueryServed Privacy

- Scope: `QueryServed` 从原始 query 文本改为 hash/scope/schema 事件合同，并保持旧事件读取兼容。
- What worked: 保留 `query_fingerprint` 作兼容字段，但新事件只写 hash；真正的新语义放到 `query_hash` / schema version / `viewer_scope`。
- What caused rework: outbox event matrix 测试原先用简单 `{` 行解析 enum；给 `events.rs` 加 impl/tests 后误读，需要把解析限定在 `pub enum WikiEvent` 块内。
- Spec changes needed: M12/suggest 对新事件先校验事件 scope，再校验 `top_doc_ids`；旧无 scope 事件只走兼容 fallback。
- Tests or reviews that caught issues: focused tests 覆盖新事件不含 raw query、旧事件 serde、CLI query outbox、M12 scoped query-history filter。
- Next plan note: 下一 PR 做 LLM governance，不混入 query/retrieval 改造。

## 2026-04-29 / Audit v2 PR 06 LLM Governance

- Scope: `wiki-cli` LLM 配置和调用边界治理：env key、provider allowlist、input/output limit、untrusted payload prompt、错误脱敏。
- What worked: 把 `ingest-llm` user prompt 统一收敛到 helper，CLI/MCP 共用同一 untrusted/redacted 格式，避免两个入口漂移。
- What caused rework: provider error body 不能只做 redaction；必须先按 `max_response_chars` 拒绝超大响应，避免异常 provider 把大 body 塞进错误路径。
- Spec changes needed: inline `api_key` 保留为 fallback；`allowed_base_urls` 为空时保留旧配置兼容，非空才强制 provider allowlist。
- Tests or reviews that caught issues: focused LLM tests 覆盖 env priority/fallback、allowlist reject、prompt redaction、oversized input、output token cap、provider error redaction/size limit；focused review 抓到 provider raw response size gap。
- Next plan note: 下一 PR 做 CI required hardening，不混入 LLM 运行时行为。

## 2026-04-29 / Audit v2 PR 07 CI Hardening

- Scope: required `quick` PR gate 加 clippy 和 cargo-deny；保留 `cargo audit` scheduled/manual artifact lane。
- What worked: 把新 required policy 塞进现有 `quick` job，避免依赖仓库 branch-protection 另行配置新 check 名称。
- What caused rework: `cargo deny` 首跑发现 `rustls-webpki 0.103.12` 新 advisory，必须顺手升级 lockfile；同时 path workspace 依赖会被 wildcard deny 误伤，PR7 只启用 duplicate/license/source/advisory policy。
- Spec changes needed: 旧 Dependency Audit PRD 只约束 `cargo audit` lane；PR7 必须明确 `cargo-deny` 是 required fast gate。
- Tests or reviews that caught issues: `cargo deny --all-features check advisories bans licenses sources` 抓到 RUSTSEC-2026-0104、license allowlist 缺口和重复依赖例外；workspace test/clippy 验证 `rustls-webpki` 升级无回归。
- Next plan note: 下一 PR 做 Production SearchPorts default，不混入 CI/workflow 继续扩展。

## 2026-04-29 / Audit v2 PR 08 Production SearchPorts

- Scope: `query` / `explain` 默认 wiki 路从 storage-backed `SqliteSearchPorts` 取候选，mempalace 继续通过 `CompositeSearchPorts` 融合。
- What worked: focused test 先清空 engine 内存 store，再确认 query/explain 仍能从持久化 snapshot 召回，直接锁住“不靠 InMemory 默认路”的行为。
- What caused rework: 初版把 `--graph-extras-file` 当 `graph_override` 传入最终 query，会替代整个 composite graph stream；同时旧逻辑允许外部文件注入 `mp_*` id。
- Spec changes needed: retrieval token quality 不在本 PR 做；CJK/Unicode 仍留给 PR9。
- Tests or reviews that caught issues: retrieval review 抓到 `graph_extras` 绕过 mempalace bank/scope 和跳过 mempalace graph；修复为拒绝 `mp_*` extras，并在 active graph stream 之后合并 extras。
- Next plan note: 下一 PR 做 CJK / Unicode retrieval，不再改 storage-vs-memory 边界。

## 2026-04-29 / Audit v2 PR 09 CJK Unicode Retrieval

- Scope: `rust-mempalace` 检索改成 Unicode token，空 token 不再 fake 成 `"memory"`，CJK query 追加 bounded LIKE fallback。
- What worked: 先加中文 temp-palace e2e，再补 unit 锁住 FTS quote、CJK fallback patterns 和 sparse embedding，能直接防止 ASCII-only token 回归。
- What caused rework: `cargo test` 仍只能接一个 test filter；聚焦多项时直接跑 package test 更稳。review 还抓到 CJK fallback 候选若先按 `id DESC LIMIT cap` 截断，会让宽泛新命中挤掉旧 exact 命中。
- Spec changes needed: 本 PR 不引入中文分词库；后续如要更高质量召回，应单独做 tokenizer / ranking spec。
- Tests or reviews that caught issues: retrieval review 抓到 fallback truncation P1；修复为全候选 rerank 后再按 user limit 截断，并让 LIKE fallback 按 exact/长 pattern score 排序。
- Next plan note: 下一 PR 做 outbox + SQLite reliability，不混入 retrieval quality 扩展。

## 2026-04-29 / Audit v2 PR 10 Outbox SQLite Reliability

- Scope: storage 层 ack 计数改为 per-consumer cursor 语义，manual ack clamp 到当前 head，SQLite open 设置 busy timeout，多步骤写事务统一走 helper。
- What worked: 直接把第二 consumer ack、manual ack 进未来、短暂 write lock 复现成 storage unit tests，避免只靠文档断言多消费者语义。
- What caused rework: 旧测试里 mempalace 第二次 ack 仍按 global processed 语义期待 `1`，新语义下应计自己从 `2` 到 `4` 的两条事件。
- Spec changes needed: `OutboxStats.unprocessed_events` 仍是 legacy/global 指标；后续若要展示每 consumer 未处理数，应新加 metrics 字段，不复用该字段。
- Tests or reviews that caught issues: reliability review 抓到 manual ack 可把 cursor 推到未来，以及 busy test 没覆盖 wrapper；`cargo test -p wiki-storage -- --nocapture` 固定了 second-consumer ack、legacy `processed_at` 兼容、future-cursor clamp 和 busy lock wait。
- Next plan note: 下一 PR 做 Vault projection safety + docs/test cleanup，不继续扩大 outbox protocol。

## 2026-04-29 / Audit v2 PR 11 Vault Docs Hardening

- Scope: Vault projection safety、docs consistency、scheduled/manual hardening lane。
- What worked: 把 engine-owned 判断从 UUID `id:` 升级为显式 managed marker，再把 stale cleanup 改成 quarantine，并在 page/root 写入目标和父目录加 guard，避免误删、覆盖手写 UUID 页或写穿 symlink。
- What caused rework: architecture 文档同时存在重复章节和旧 Notion 状态；review 还抓到 cleanup guard 不等于 write guard、target guard 不等于 parent/root guard，PR11 必须把文档一致性和写路径数据保护都当验收项。
- Spec changes needed: 旧无 marker projection 文件保守保留；如需清理历史文件，应另开 dry-run cleanup spec。
- Tests or reviews that caught issues: focused projection tests 覆盖 empty slug、YAML escape、managed marker、quarantine、write collision、symlinked subdir、root index symlink、手写 UUID 保留；slow lane smoke 覆盖 perf/MCP boundary/DB/CJK/bank-scope 矩阵。
- Next plan note: Audit v2 新增剩余项已收敛，后续进入 CI/PR closeout。

## 2026-04-28 / PR #78 CLI Command Modularization phase 1

- Scope: 先搬低风险命令域：`schema-validate`、`llm-smoke`、outbox export/ack。
- What worked: 保留 clap enum 和大 dispatcher，先建立 `commands/` 模块壳，避免参数行为漂移。
- What caused rework: `cargo test` 多 filter 不能一次传多个测试名；需要拆成单 filter 命令。
- Spec changes needed: phase 2 才拆 dispatcher/shared config；phase 1 不移动 clap 定义。
- Tests or reviews that caught issues: outbox cursor focused test 和 schema_validate integration tests 固定搬迁后行为。
- Next plan note: 下一 PR 拆 dispatcher/shared config，并补命令级 smoke。

## 2026-04-28 / PR #79 CLI Command Modularization phase 2

- Scope: 拆 no-engine dispatcher 和 shared runtime setup，保留 clap enum 与 engine-backed command match。
- What worked: 先搬早期分支和 repo/schema/engine setup，避免一次性移动所有业务命令。
- What caused rework: `cargo test -p wiki-cli --lib ...` 不适用于 binary-only crate；integration 文件要用 `--test <name>`，不能只把文件名当 filter。
- Spec changes needed: 后续若继续瘦身 `main.rs`，应按命令域逐个搬 business handler，不再混入 clap 行为修改。
- Tests or reviews that caught issues: clippy/compile warning 提示 destructuring 可用 shorthand，CLI smoke 固定 global arg 位置。
- Next plan note: 下一项是 Time Library Unification。

## 2026-04-28 / PR #80 Time Library Unification

- Scope: 移除剩余 direct `chrono`，统一到 `time::OffsetDateTime` 当前时间 RFC3339 格式化。
- What worked: 先用 `rg` 确认 `chrono` 只用于 `Utc::now().to_rfc3339()`，因此可直接替换，不需要保留 mempalace 边界。
- What caused rework: `time::format` 返回 `Result`，写入路径和默认时间分支都应走可传播 helper，避免为默认值闭包引入 panic wrapper。
- Spec changes needed: boundary decision 写清楚：不保留 `chrono` 类型边界，因为没有外露 chrono API。
- Tests or reviews that caught issues: live bridge feature 测试确保 optional dependency 从 `chrono` 切到 `time` 后仍能编译。
- Next plan note: 下一项是 M12 executor dry-run planner。

## 2026-04-28 / PR #77 Row-level Wiki State Storage cutover

- Scope: `load_snapshot` 切为 rows-present 时 row-primary，rows 为空时 blob fallback。
- What worked: cutover 不需要改变 `WikiRepository` trait；在 `SqliteRepository` 内部重排读取优先级即可。
- What caused rework: recovery 不能靠口头说明，必须保留 blob 双写和 read-only verification API 给后续生产核对。
- Spec changes needed: 兼容层删除仍需等生产验证；本 PR 不删除 `wiki_state`。
- Tests or reviews that caught issues: focused storage tests 证明 row-primary 胜过 stale blob、rows absent 时 fallback、verification 能报 match/mismatch。
- Next plan note: 下一项进入 CLI Command Modularization phase 1，先拆低风险命令域，不动 clap 行为。

## 2026-04-28 / PR #76 Row-level Wiki State Storage migration

- Scope: 为 `wiki_state` 增加 row-level mirror，但保留单行 JSON blob 为主读路径。
- What worked: dual-write 放进现有 snapshot/outbox transaction inner，embedding / alias / Notion index 组合写入自动继承同一回滚边界。
- What caused rework: edges 没有稳定 id，row-level mirror 必须用 ordinal key + position 保留顺序，不能假造业务 id。
- Spec changes needed: cutover 仍是下一 PR；本 PR 只提供 fallback row load，不把 rows 设为默认真源。
- Tests or reviews that caught issues: focused storage tests 固定 dual-write、stale row 清理、row insert failure rollback。
- Next plan note: 下一 PR 才做 row-level primary read、迁移验证和兼容层清理计划。

## 2026-04-28 / PR #75 J14 Semantic Fusion Benchmark

- Scope: 给 LongMemEval runner 增加 `--compare-semantic-fusion`，同一 case 同时报告 `query_baseline` 与 `semantic_fusion`。
- What worked: 复用 J13 runner 和 artifact 合同，只新增 `metrics_by_variant` / `variant_results`，避免新建一套 workflow。
- What caused rework: 真正外部 embedding 会引入 key、费用和限流；本 PR 先用本地 sparse semantic fusion 做评估 lane。
- Spec changes needed: J14 低分仍只写报告，broken run 才 fail；不能变成 PR required check。
- Tests or reviews that caught issues: fixture fake CLI 读取 per-variant `config.json`，验证 semantic_fusion 与 query_baseline 指标分开输出。
- Next plan note: 下一项进入 `Row-level Wiki State Storage migration/dual-write`，先做兼容表和双写，不直接 cutover。

## 2026-04-28 / PR #74 Contradiction Scan Scaling

- Scope: 把 `naive_contradiction_pairs` 从 visible all-pairs 改为 stale 预过滤、scope 分桶、contradiction signal index 和每 claim 256 候选上限。
- What worked: 保留 `contradicts_heuristic` 作为最终判断，只改候选生成，避免改变 `ContradictionHint` API。
- What caused rework: all-pairs 里 stale 是内层才跳过；提前过滤后才能真正减少候选量。
- Spec changes needed: 明确 cross-scope contradiction 不再配对，符合现有 scope isolation 规则。
- Tests or reviews that caught issues: focused kernel tests 覆盖 stale/scope 过滤和无信号 claim 跳过。
- Next plan note: 下一项进入 `J14 Semantic Fusion Benchmark`，作为评估 lane，不让低分阻断 CI。

## 2026-04-28 / PR #73 Embedding ANN Implementation

- Scope: 在 `ann-embed` 后实现本地 locality-bucket shadow index、bounded candidate search、same-transaction index maintenance、rebuild 和 full-scan fallback。
- What worked: 先保留 public `search_embeddings_cosine` 合同，内部只换 backend；默认 build 继续 exact full scan，feature build 才走 bounded candidate path。
- What caused rework: feature 模式下旧 ranking 测试发现候选不足会少返回结果；修成候选不足时 warning + fallback full scan，保持兼容。
- Spec changes needed: `sqlite-vec` 降为未来可选替换；当前完成路径是 DB-local shadow index，避免 native extension 发布问题。
- Tests or reviews that caught issues: `cargo test -p wiki-storage embedding_` 与 `cargo test -p wiki-storage --features ann-embed embedding_` 覆盖 default、feature、fallback、rebuild、delete cleanup。
- Next plan note: 下一项进入 `Contradiction Scan Scaling`，用 stale 预过滤、scope 分桶和 bounded candidates 降低 O(n²)。

## 2026-04-28 / PR #72 Embedding ANN Spike

- Scope: 为 C16B 增加 `ann-embed` feature gate、backend dispatch 和 CI smoke；默认仍走 exact full scan。
- What worked: 先落无 native 依赖的 gate，让后续 ANN 实现只填 backend，不再同时争论构建/发布边界。
- What caused rework: 不应在 spike PR 直接引入 sqlite native extension；否则 quick CI 和本机环境会被平台依赖拖住。
- Spec changes needed: `embedding-ann-index` 设计明确 `sqlite-vec` 是优先目标，但真实 DDL/load/search 留给 implementation PR。
- Tests or reviews that caught issues: feature-gate smoke 必须在 `--features ann-embed` 下编译并回落 full scan。
- Next plan note: 下一项进入 `C16B Embedding ANN implementation`，只在 `ann-embed` gate 后面加真实 bounded search。

## 2026-04-28 / PR #71 Embedding Tx Atomicity

- Scope: 新增 snapshot + outbox + embedding rows 单事务提交路径，并把 CLI/MCP/compiler 的 vector 写入口切过去。
- What worked: 先把 embedding 写入包装成 `EmbeddingWrite`，调用方只负责在 commit 前生成向量，storage 负责事务边界。
- What caused rework: MCP 旧逻辑是 best-effort embedding，会静默吞掉失败；为了保证 atomicity，vectors 开启时必须把失败升格为写入失败。
- Spec changes needed: ANN index 仍独立；本 PR 只修 `wiki_embedding` blob 行与 snapshot/outbox 的事务一致性。
- Tests or reviews that caught issues: storage 触发器强制 embedding insert 失败，验证旧 snapshot 保留、outbox 不落半截、embedding 不落孤儿行。
- Next plan note: 下一项进入 `C16B Embedding ANN spike / feature gate`，先锁技术路径和 feature gate，不默认引入 native extension。

## 2026-04-28 / PR #70 Benchmark Reproducibility

- Scope: 给 `rust-mempalace bench --mode random` 增加 `--seed`，并把 seed 写入 `benchmark_runs`、CLI 输出和 benchmark report。
- What worked: 把随机选样逻辑拆成小 helper 后，确定性可以用纯单元测试锁住，不依赖检索命中偶然性。
- What caused rework: `BenchMode` 原本不需要比较，新增 fixed+seed fail-fast 后必须补 `PartialEq`。
- Spec changes needed: J14 仍独立；本 PR 只保证现有 random benchmark 可复现，不引入 semantic fusion。
- Tests or reviews that caught issues: focused compile/test 抓到 `BenchMode` derive 缺失；e2e 覆盖 seed 输出、DB 存储和 fixed+seed 拒绝。
- Next plan note: 下一项进入 `Embedding Tx Atomicity`，把 embedding 写入纳入 snapshot/outbox 事务边界。

## 2026-04-28 / PR #68 Notion Archived Source Retirement Audit Plan

- Scope: 新增 `notion-archived-retirement plan`，从 `notion_page_index` 拉取 Notion archived/in_trash 状态，生成 DB-first dry-run JSON/Markdown retirement plan。
- What worked: 先把 apply 明确排除，只做报告，让 Notion API、DB index、source evidence 三者可审计对齐。
- What caused rework: `wiki-storage` 生产代码新增 UUID 解析后，`uuid` 不能只放 dev-dependencies；依赖边界要随代码路径同步调整。
- Spec changes needed: Apply PR 必须读取 plan JSON，只处理 `apply_safe=true`，并按 DB -> Vault projection -> Mempalace consumer 顺序执行。
- Tests or reviews that caught issues: focused tests 覆盖 storage index listing、Notion archive state client、plan report builder；`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、production `--limit 3` smoke 均通过。
- Next plan note: 下一 PR 做 `Notion Archived Source Retirement apply`，保持 DB-first，不手删 Markdown。

## 2026-04-28 / PR #69 Notion Archived Source Retirement Apply

- Scope: 新增 `notion-archived-retirement apply --plan <PATH>`，默认 dry-run；`--apply` 时只处理 `apply_safe=true`，先退役 DB source 和 `notion_page_index`，再删除匹配 Vault source 文件。
- What worked: apply 前重新校验 `source_id`、`source_uri`、`db_id`、Notion page ID，避免旧 plan 误删当前 source。
- What caused rework: Source 文件不是 `write_projection` 管理，apply 需要单独按 frontmatter 身份清理 `sources/**.md`，不能只依赖 page projection。
- Spec changes needed: Source retirement 不删除 compiled pages；若要清理已编译知识页，必须另起 page-level plan。
- Tests or reviews that caught issues: CLI integration test 覆盖 dry-run 不变更、`--apply` 退役 DB/index/Vault；storage test 覆盖 snapshot + Notion index delete 同事务。
- Next plan note: 下一项进入 `Benchmark Reproducibility`，先给 random benchmark 加 seed，再做 embedding/ANN 线。

## 2026-04-25 / PR #16 M12 Strategy Suggestions

- Scope: 新增只读 `wiki-cli suggest`，输出 text/JSON，并在显式 `--report-dir` 时生成同源 JSON/Markdown suggestion report。
- What worked: 先做白话架构对话，把 “suggest 只诊断派单，不执行” 和 “JSON 是真源，Markdown 只给人看” 定清楚，后续实现分工更稳。
- What caused rework: reviewer 抓到 report_id 秒级时间会覆盖历史、Manual fix 默认过宽、`--report-dir` 默认目录语义不完整；这些都应在 spec review checklist 里提前列成边界测试。
- Spec changes needed: M12 spec 需要保留后续 internal operator/executor、dashboard latest suggestion report、QueryServed scope/hash schema 改进为 deferred follow-ups。
- Tests or reviews that caught issues: Reviewer D 的 focused review 覆盖只读边界、JSON/Markdown 同源、QueryServed scope-safe、execution_policy 映射；本地 `cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 和 GitHub `quick` CI 均通过。
- Next plan note: Batch-3 剩余主线转向 J13 LongMemEval Auto Benchmark；M12 后续增强应独立规划 internal operator/executor，不要混进 suggest 首版边界。

## 2026-04-25 / PR #19 J13 LongMemEval Auto Benchmark

- Scope: 新增 `rust-mempalace` 本地检索基线评测 lane，包括 fetch/cache script、stdlib-only runner、nightly/weekly GitHub workflow、fixture tests、artifact contract 和 handoff/review 文档。
- What worked: 白话架构先把 J13 定成“定期考试/体检”，并把 J14 Semantic Fusion Benchmark 拆成后续模块，避免首版混进外部 embedding、key、费用和限流问题。
- What caused rework: 专门 review subagent 抓到 fake CLI 测试遮住真实检索契约、runner 没有 per-command timeout、workflow `fixture` mode 仍会 fetch 远程数据、tasks 状态滞后；这些以后应直接写进 review checklist。
- Spec changes needed: J13 spec 应保留 `R@1/R@5/MRR`、runtime health、低分不 fail、broken run fail、J14 启动 gate。J14 需等 7 份 nightly、1 份 weekly full、artifact 稳定、full run 耗时明确后再开。
- Tests or reviews that caught issues: Subagent C focused/integration review 抓到 P2/P3；本地 `python3 tests/longmemeval_runner_test.py` 覆盖 fake CLI metric math 和真实 `rust-mempalace` smoke；`cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 和 GitHub `quick` CI 均通过。
- Next plan note: Batch-3 P2 maturity 已完成主线。下一步先观察 J13 scheduled artifacts；不要启动 J14，除非 J13 有足够报告证明语义融合值得接入。

## 2026-04-25 / PR #25 C16A Atomic Snapshot + Outbox

- Scope: 新增 `WikiRepository::save_snapshot_and_append_outbox`，把 `wiki_state` snapshot 和本次 outbox append 放进同一 SQLite transaction；CLI / MCP / vault-backfill 写路径切到原子提交。
- What worked: 先把 C16 拆成 C16A 存储一致性和 C16B ANN 性能，避免把 transaction API 变更和 SQLite extension 选择混在一个 PR。
- What caused rework: 合并前 roadmap / PRD / spec 已标 “in progress”，合并后仍需单独回填；以后 PR body 或 handoff 应提醒 “merge 后状态 PR”。
- Spec changes needed: `persist-snapshot-outbox` 设计锁定 option A：trait 方法 + `BEGIN IMMEDIATE`；C16B 仍保持独立 spec。
- Tests or reviews that caught issues: rollback 测试用 SQLite trigger 强制 outbox insert 失败，验证旧 snapshot 保留且 outbox 不落半截；本地 `cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 和 GitHub `quick` CI 均通过。
- Next plan note: 下一步优先跑生产 vault 的 B1 audit；C16B ANN index 如需推进，单独开新分支和设计评审。

## 2026-04-25 / Production Vault Backfill + Palace Init

- Scope: 对 `/Users/mac-mini/Documents/wiki` 执行生产 backfill，把历史 source/page 登记进 `wiki.db`，再用 `palace-init` 同步到 `/Users/mac-mini/Documents/wiki/.wiki/palace.db`。
- What worked: 先跑 dry-run 和 `/tmp` 小样本 apply，再备份生产 vault，最后执行全量 apply；这个顺序让批量改 4475 个 Markdown frontmatter 的风险可控。
- What caused rework: query/explain 验证本身会追加 `query_served` outbox；验证后要再跑一次 `consume-to-mempalace`，把 mempalace consumer progress 补到 head。
- Spec changes needed: 生产数据初始化任务要把 “验证命令也可能产生 outbox” 写进 checklist。
- Tests or reviews that caught issues: `vault-audit`、`vault-backfill --apply`、frontmatter count、DB snapshot count、outbox count、`palace-init` report、fusion `query/explain --palace-db` 均通过。
- Next plan note: 生产 backfill 已完成；下一步是 B5 orphan governance，基于新 audit 报告处理 4 个 orphan candidates 和 unsupported frontmatter，不要重复跑全量 backfill。

## 2026-04-25 / B5 Orphan Governance

- Scope: 新增只读 `wiki-cli orphan-governance`，读取生产 `vault-audit.json`，生成 JSON/Markdown sibling report，把 4/12/5/16 四类审计发现分到 human-required、agent-review、future-auto-fix lane。
- What worked: 先用白话架构锁定“报告可写、vault 不清理”，实现就能保持 DB/outbox/palace 零触碰。
- What caused rework: reviewer 抓到旧/空 audit 会被默认成 0，以及 report-dir symlink 可逃逸；以后 report command 的 path gate 要直接测 malformed input 和 symlink escape。
- Spec changes needed: 后续若要修 `status` 或 `compiled_to_wiki`，先让 `vault-audit` 输出 path-level arrays，再更新 B5 spec 并让用户确认 apply mode。
- Tests or reviews that caught issues: 独立 review subagent 抓到 2 个 P2；新增 malformed audit 与 symlink escape tests；最终需跑 `fmt/test/clippy` gate。
- Next plan note: B5 v1 只给治理报告。不要在本 PR 里清理 `_archive`、改 frontmatter、重跑 LLM 或移动历史文件。

## 2026-04-26 / DB/Vault/Palace Consistency Governance

- Scope: 新增 `consistency-audit` / `consistency-plan` / `consistency-apply`，以 `wiki.db` 为原点审计 Vault 与 Mempalace page 镜像，再按白名单 dry-run/apply。
- What worked: 先真实跑生产 audit/plan/dry-run，再在 Git 保护下 apply；最终 DB 应用 305 个旧 Notion 导出链接修复，Mempalace replay 189 个 page，后验 plan 可执行动作归零。
- What caused rework: 初版 audit 把所有 DB page 都要求进 Mempalace，误报 index/lint-report 等非 eligible page；真实 apply 还暴露全量 Vault projection 会重写过多页面并丢迁移 frontmatter。后续 apply 类命令必须优先做 targeted projection，并保留现有 frontmatter。
- Spec changes needed: Mempalace audit 必须写清 “source drawers out of scope” 和 “只有 summary/concept/entity/synthesis/qa page 进入 palace”；Vault projection 命名和 frontmatter 保留规则必须和生产迁移格式一致。
- Tests or reviews that caught issues: 生产复查 audit 抓到 Mempalace eligibility 误报；真实 apply 抓到 projection 重写风险；新增 ineligible page、targeted projection 不新建缺失旧页、保留 frontmatter 的回归测试；本地 `cargo test -p wiki-cli --test consistency`、`cargo test -p wiki-kernel`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 和 GitHub `quick` CI 通过。
- Next plan note: Notion archived 状态还没有同步到本地退役流程；已知样本 `sources/wechat/微信公众号文章链接汇总.md` 在 Notion 为 `is_archived=true`，但本地仍在 `wiki.db.sources` 和 Vault。下一轮要做 DB-first archived source retirement，不要手删 Markdown。

## 2026-04-26 / CR-01 Code Review Fixes (PR #34)

- Scope: 修复全库代码审查发现的 Critical/High/Medium/Low 问题，共 8 个模块（快照序列化确定性、SourceIngested unresolved 语义、flush_outbox drain 精度、save_snapshot 事务包装、notion_uuid 锚定提取、url_index 重复 URL 检测、benchmark hits 真实存储、cleanup 保护文档）。
- What worked: 从 explore-mode review 产出结构化问题列表，再逐条对照代码确认后才动手，避免基于 review 描述直接猜测实现；每个模块都有独立测试覆盖；白话架构先确认延后范围边界后再写 spec，让实现范围保持紧凑。
- What caused rework: 新增测试插入位置破坏了相邻的 `#[test]` 函数头（`unresolved_supersede_scope_is_not_dispatched` 的 `fn` 头被消费），导致括号不匹配；下次插入测试时应在上下文中显式包含被插入位置的完整 `#[test]\nfn` 标记行，而非只用函数体开头匹配。clippy `-D warnings` 在 Rust 1.95 抓到两处新 lint：`needless_borrows_for_generic_args`（format! 借用）和 `attempt_to_mutate_range_bound`（range 变量在循环内变更）。
- Spec changes needed: 设计文档 M4 的代码示例细节与最终实现有细微差异（`strip_suffix` 逻辑路径调整）；伪代码级设计文档只做方向参考，实现以代码为准，不需要每次精确同步。
- Tests or reviews that caught issues: clippy 抓到两处编译时 lint；新增 `source_ingested_unresolved_scope_counted_as_unresolved`、`source_ingested_filtered_scope_counted_as_filtered` 直接覆盖 M2 语义修正；`to_snapshot_is_deterministic` 验证 M1。
- Next plan note: 四项 CR-01 延后 follow-up 已写入 roadmap（MCP Vault Sync、Outbox Consumer Cursors、Embedding Tx Atomicity、Benchmark Reproducibility）。各项影响面窄，适合独立小 PRD，不建议合并批次。embedding tx 需存储层改造，风险最高，建议最后处理。

## 2026-04-26 / Notion Incremental Sync (PR #36)

- Scope: 新增 `wiki-cli notion-sync` 子命令和 `AutomationJob::NotionSync`，通过 Notion API 增量拉取 X书签文章数据库和微信文章数据库到 `wiki.db`；新增 `notion_sync_cursors` / `notion_page_index` 两张表；速率限制 + 429 重试；`NotionWriteBackClient` trait 默认关闭。
- What worked: 白话架构对话先把「增量去重」「速率限制」「写回接口先关闭」三个关键约束定清楚再写 spec，实现阶段没有返工。T1→T2→T3→T4→T5 的模块顺序依赖关系清晰，每个模块有独立测试，focused review 逐一把关效果好。测试用 in-memory stub 替代真实 HTTP（T4）加上 mockito mock server（T2/T3）完全不需要真实 Notion token 就能跑通。
- What caused rework: 云端 Cargo 默认版本（1.83.0）拉依赖时遇到 `time-macros` edition2024 解析失败，需要全程加 `+stable` 绕过；这是云端环境特有问题，本地 Mac mini 不受影响。`clippy -D warnings` 下的 dead_code 处理：`last_edited_time` 字段目前只在测试用、`DomainSchema` use 放在了非 test 作用域、`from_env()` 方法被认为未调用——这三处都需要调整，以后新增只在测试里用到的 pub 字段/方法时，应提前加 `#[allow(dead_code)]` 或移到 `#[cfg(test)]` 作用域。现有 automation job 列表断言测试（固定列表 assert_eq!）在新增 job 时必然失败，需同步更新。
- Spec changes needed: design.md §7 自动化 job 注册描述了 `short_circuit_on_failure` 字段，但实际 `AutomationJobSpec` 没有该字段（只有 `in_daily` + `requires_network`）；spec 描述比代码超前，以代码为准，spec 可在合并后修正。
- Tests or reviews that caught issues: clippy `-D warnings` 抓到 dead_code 和 unused import 4 处；T4 dry_run 测试确认了 cursor 不更新的边界；integration review 确认了 `notion://` URI 与 `vault_audit`/`vault_backfill` 的 `file://` 路径完全隔离。smoke test 用真实 NOTION_TOKEN 验证了 dry-run 返回 782 + 482 = 1264 条。
- Next plan note: PR 合并后在本地 Mac mini 执行首次真实同步（`notion-sync --db-id all`），再跑 `batch-ingest` 编译新文章。后续优先做 Notion Archived Source Retirement（识别本地已有但 Notion 已归档的 source，生成退役计划），再考虑 Scheduled Vault Reports。

## 2026-04-26 / PR #38 Notion Incremental Sync Post-merge Cleanup

- Scope: 对 PR #36 实现结果做补齐验收，修复文档与 automation 约束描述偏差（字段名、自动化 spec 字段、批量索引入库语义），并补全 post-merge 状态回填。
- What worked: 先快速补齐 spec 与 code 的命名一致性，再一次性同步 `requirements/design/tasks/implementation/roadmap/lessons` 文档状态，避免生产实现与文档长期偏离。
- What caused rework: PR #36 初稿留下 `notion_sync_state`、`run_in_daily_chain`、`short_circuit_on_failure` 等过时表述，触发回填工作；`branch` 记录也需更新为当前维护链路。
- Spec changes needed: spec 层已改为与实现字段对齐；后续仅保留 `内容更新语义` 的产品化扩展，不再将已合并项作为未完成项。
- Tests or reviews that caught issues: 专门 review 发现 spec 与代码字段偏差；`cargo fmt --all -- --check`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` 均已通过。
- Next plan note: 未完成项优先转 `Notion Archived Source Retirement` 与 `Notion Incremental Sync` 续 PRD（更新语义），按 DB-first 治理链路推进。

## 2026-04-27 / PR #42 Notion Source Vault Projection

- Scope: 把 DB-backed `notion://` sources 投影为 Obsidian 可见的 `sources/x` 和 `sources/wechat` Markdown；补齐 Notion block 正文抓取、`--refresh-existing`、Obsidian-safe tags、automation existing refresh。
- What worked: 先让用户在 Obsidian 看真实结果，再用 dry-run/idempotence 检查闭环；真实生产修复后最终 `notion-source-vault-sync --dry-run --refresh-existing --repair-tags` 为 planned 0 / tags_rewritten 0，说明 DB 与 Vault 投影已稳定。
- What caused rework: 初版只取 Notion database properties，导致新导入 source 没有正文；原始 Notion 标签直接写入 Obsidian tags，`Apache2.0` 这类标签显示异常。以后外部内容同步必须把“正文来源”和“目标系统标签语法”写进首版验收。
- Spec changes needed: Notion incremental sync spec 必须明确 page blocks 是 source body 的组成部分；automation `notion-sync` 需要在 cursor window 内刷新已有 source，避免自动任务长期保留旧正文/tags。
- Tests or reviews that caught issues: 用户 Obsidian 复查抓到 tag 和正文问题；新增 projection idempotence、refresh existing、duplicate Notion UUID/source_id 匹配测试；本地 `cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`git diff --check` 和 GitHub quick CI 通过。
- Next plan note: 现在完成的是 raw source 入库与 Vault 可见，不是 Wiki 编译。下一轮应单独走 Notion Source Compilation：小样本编译 `compiled_to_wiki: false` source，再批量编译、验证 query/mempalace，不要把 archived retirement 混进同一 PR。

## 2026-04-27 / PR #44 Production Wiki Compiler

- Scope: 把 `batch-ingest` 升级为本地 Wiki Compiler，支持 raw source -> summary + concept/entity pages -> Vault projection -> outbox 的 Notion-equivalent 合同，并补齐 PRD/spec/handoff。
- What worked: 先对照 Notion Wiki Compiler 设置页做 PRD/spec，再用 subagent 分拆 plan contract 和 runner，最后用 review subagent 抓生产风险；这个顺序避免了继续沿用旧的 summary-only 业务流。
- What caused rework: review 抓到同标题不同 source 在 Vault 投影会互相覆盖、YAML block tags 丢失、relationship lookup 未按 scope、rich `summary.confidence` 没写入 page metadata，以及中断重跑可能重复 raw source；这些都应成为 compiler 类模块的固定 review checklist。
- Spec changes needed: PRD 已完成实现部分，但 production tiny sample 仍是单独 operational gate；不要把 “代码已合并” 误写成 “生产闭环已跑通”。
- Tests or reviews that caught issues: integration review subagent 抓到 P1/P2/P3；新增 wiki_compiler、projection duplicate title、rich fixture、frontmatter metadata 回归测试；本地 `cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`git diff --check` 和 GitHub CI 均通过。
- Next plan note: 该 operational gate 已由 PR #46/#47/#54 后续链路完成。当前不再回到 tiny sample 阶段；继续小批量 production scale-up。

## 2026-04-27 / PR #47 Compiler Canonicalization v2

- Scope: 在 compiler draft 和 DB 写入之间加入 canonical resolver；新增 bounded candidate retrieval、`wiki_canonical_alias`、small LLM fallback、temp-vault regression，以及 machine-owned `deferred_resolutions` JSON。
- What worked: 用户的 Vault 验收标准比命令清单更有效；用 main vs v2 的临时 Vault 可视化对比，直接看出旧结果会产生 broken wikilink/incomplete entity，而 v2 把模糊项 defer 出 active graph。
- What caused rework: “模糊项人工确认”这个表述不符合产品方向；正确语义是机器后置治理，模糊项不污染 active graph，但也不甩给人工，而是进入 resolver/lint/fixer 可消费队列。
- Spec changes needed: 以后 compiler/fixer 相关 spec 要明确区分 `duplicate`、`ambiguous/deferred`、`noise/ignore` 三类；deferred 产物必须是 machine-readable artifact，不只是 Markdown warning。
- Tests or reviews that caught issues: live temp-vault smoke 抓到 LLM 返回 `null` 导致 parser 失败；lint 抓到 unresolved `related_names` 被直接写成 wikilink 会产生 broken link；focused security/architecture review 抓到 alias poisoning 与 transaction split 风险。本地 `fmt`、`wiki_compiler`、`cargo test --workspace`、`clippy -D warnings`、`git diff --check` 和 GitHub quick CI 均通过。
- Next plan note: Compiler Deferred Resolution Agent 已在 PR #54 完成；后续重点转为 production compiler 小批量 scale-up，并持续检查 deferred、断链、重复页和 Vault 内容质量。

## 2026-04-27 / PR #54 Compiler Deferred Resolution Agent

- Scope: 新增 `compiler-resolve-deferred`，消费 compiler run JSON 的 `deferred_resolutions`，机器判定 alias existing / safe create / ignore noise / keep deferred；无人工 lane。
- What worked: 直接用 temp X + WeChat apply smoke 验证 DB -> Vault -> Mempalace -> lint/audit，省掉口头验收步骤。
- What caused rework: 首版 safe create 的来源引用写成 `[[摘要：...]]`，临时库没有 summary 页，lint 抓到 broken wikilink；后置 resolver 创建页时不能假设 summary 已存在。真实 scale-up 后又暴露 `Cloud Run Instances` 这类 title-related single candidate 和 `Magnus M ü ller` 这类 Unicode spacing 问题，需要 PR #54 hardening。
- Spec changes needed: deferred candidate 必须带 `page_id`；旧 report 只能在 scope + title + entry_type 唯一时 fallback。低置信项应继续 machine-deferred，不进入人工 lane，也不直接污染 active graph。
- Tests or reviews that caught issues: 新增 `compiler_resolve_deferred` 单测覆盖 alias、ambiguous、noise、allow-create、title-related alias、weak-context create、Unicode title cleanup；temp X + WeChat smoke 确认 no `page.broken_wikilink`；真实 production apply 后复查无新增断链、无新增 duplicate concept/entity group，`wiki.db` / `palace.db` integrity 均为 `ok`。
- Next plan note: 系统开发闭环已完成。下一步不是再补 compiler 架构，而是进入 archived source 生命周期治理，保持 compiler scale-up 生产闭环后置机制（每批固定执行 deferred apply、Mempalace consume、lint/audit、重复页检查和 Vault spot-check）。

## 2026-04-28 / PR #56 Compiler Boundary Hardening

- Scope: fix production compiler boundary failures（LLM JSON 边界、标题注入、短文跳过、percent 解码）并验证全量小批量 scale-up 闭环，不新增功能。
- What worked: 先在 `batch-ingest` 循环中修复 parse 边界（JSON 模式请求、finish_reason 截断保护、标题消毒）后再回填，未引入别的策略改动即可完成剩余编译。
- What caused rework: 边界问题会在小批量里放大到 0.0；同一类问题若没有被单测覆盖，容易被生产内容再次触发。
- Spec changes needed: 与 PRD 的“先完成治理闭环再大规模”一致，不新增 `compiler` prompt alias 清洗逻辑；继续把 `deferred_resolutions`、lint/audit、vault spot-check 留在生产链路。
- Tests or reviews that caught issues: focused review + temp-vault smoke + 真生产分批核对，`wiki.db`/`palace.db integrity=ok`，重复 concept/entity 与断链无新增，`compiled_to_wiki` 运行后归零。
- Next plan note: 进入 P1 Orphan/Archived Source Retirement；继续保持生产编译链路稳定。

## 2026-04-27 / Compiler Model Candidate Trial

- Scope: 用临时 DB/Vault 对比 compiler 模型，不触碰真实 `/Users/mac-mini/Documents/wiki`。
- What worked: `deepseek/deepseek-v4-flash` 在 OpenRouter 小 JSON smoke 1.83s；同一 `Avatar V` full compiler 成功 117.0s；再跑 3 条 X source 全部成功，单条 89.5-99.1s。
- What caused rework: 速度和 JSON 稳定性够用，但仍会产生归一化质量问题：重复近义概念、主实体 deferred、人名 Unicode 标题异常。
- Spec changes needed: 模型替换不能只看成功率和速度，compiler temp-vault regression 要加入“主实体页存在、近义 concept 不重复、人名/文件名 Unicode 正常化”验收项。
- Tests or reviews that caught issues: 临时 3-source smoke 抓到 `自愈式浏览器自动化` / `自愈浏览器自动化` 重复，以及 `Magnus M ü ller` 标题异常；`compiler-resolve-deferred` dry-run 仍保持低置信 deferred。
- Next plan note: 记录 `deepseek/deepseek-v4-flash` 为待选模型；生产默认暂不切换，先补 resolver/fixer 归一化能力。

## 2026-04-28 / Audit Report Hardening

- Scope: 按 Notion 全方位代码审计报告修复可一轮安全落地项：MCP 输入上限、LLM plan 后置校验、脱敏规则、outbox 独立 flush batch transaction、`rust-mempalace` FTS query quoting、automation DB integrity、single-writer docs、MCP API reference。
- What worked: 先把报告项映射到真实代码，再区分“小补丁可修”和“需独立架构迁移”；这避免把 `wiki_state` 行级迁移、ANN、`main.rs` 大拆分硬塞进同一 PR。
- What caused rework: 并行跑多个 `cargo test` 会互相等待 package/artifact lock，输出噪音大；以后 cargo 验证默认顺序跑，只有纯读命令并行。clippy 抓到新 MCP loop 可写成 `while let`。
- Spec changes needed: 安全/可靠性审计类修复也要补 PRD/spec/handoff；否则“按审计报告修复”很容易失去剩余架构项的明确落点。
- Tests or reviews that caught issues: 新增 redaction、LLM bounds、MCP line cap、FTS quoting、automation health integrity 测试；`cargo test --workspace`、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings` 通过。
- Next plan note: 剩余审计项应拆成独立 PRD/roadmap 条目：row-level storage、ANN vector path、CLI command modularization、MCP typed errors、multi-process writer lock/lease、contradiction scaling、reliability/fuzz/perf tests、dependency audit、time library unification；不要和 production compiler scale-up 混做。

## 2026-04-28 / MCP Typed Errors

- Scope: 把 MCP `tools/call` 的字符串错误映射成 JSON-RPC typed error shape：`error.code` + `error.data.kind`。
- What worked: 在 MCP edge 私有化 `McpToolError`，不动 core engine/storage error 类型，改动面小且后续 MCP Vault Sync 可复用。
- What caused rework: 参数错误要保留旧 human message，同时新增 machine kind；测试应同时断言 code、message、kind。
- Spec changes needed: `docs/mcp-api-reference.md` 要列出 error kind 表，否则客户端仍只能靠 message parse。
- Tests or reviews that caught issues: `cargo test -p wiki-cli mcp::tests:: -- --nocapture` 覆盖 unknown method、missing arg、tag invalid item 和旧 MCP scope 行为。
- Next plan note: 下一项优先做 `MCP Vault Sync`，直接复用本轮 typed errors 作为 projection 失败的诊断出口。

## 2026-04-28 / MCP Vault Sync

- Scope: MCP 写工具在 `--sync-wiki` 启用时自动刷新 Vault projection。
- What worked: 入口只在 `sync_wiki` 为 true 时传 `wiki_dir`，写路径复用一个 `save_flush_and_project()`，避免每个工具重复逻辑。
- What caused rework: 不能只看 `--wiki-dir`；必须同时要求 `--sync-wiki`，否则会破坏“仅配置路径不代表写 Vault”的 CLI 语义。
- Spec changes needed: MCP API reference 要把 projection side effect 写进 Runtime Rules。
- Tests or reviews that caught issues: focused tests 覆盖 write_page projection 和 projection 失败的 typed `storage_error`。
- Next plan note: 后续 `Outbox Consumer Cursors` 不应把 Vault projection 和 Mempalace consumption 混在同一协议里。

## 2026-04-28 / Outbox Consumer Cursors Phase 1

- Scope: 新增 storage-level consumer-scoped outbox export API，不改变 CLI 默认导出/ack 行为。
- What worked: `wiki_outbox_consumer_progress` 已存在；Phase 1 只需要把 cursor 读取和 NDJSON export 包成一个明确 API。
- What caused rework: roadmap 原文把 schema/API 和 cutover 写在同一项；实现时必须拆开，避免一次 PR 同时改协议和调用方行为。
- Spec changes needed: outbox docs 要明确 legacy ID export 与 consumer-scoped export 同时存在。
- Tests or reviews that caught issues: focused storage test 覆盖 consumer A ack 后 consumer B 仍从自己的 cursor 导出。
- Next plan note: Phase 2 再切 `consume-to-mempalace` / export CLI 默认语义，并保留 `--last-id` manual override。

## 2026-04-28 / Outbox Consumer Cursors Phase 2

- Scope: `export-outbox-ndjson-from` 和 `consume-to-mempalace` 切到 consumer cursor 默认语义，`--last-id` 保留为 legacy/manual floor。
- What worked: 把 cursor export + floor 逻辑收进一个 CLI helper，避免 export 命令和 mempalace consumer 分叉实现。
- What caused rework: 初版 patch 切掉了 `stats` 变量但 ack 仍引用 `stats.head_id`；helper 返回 `head_id` 后，ack 边界重新收敛到同一份 export metadata。
- Spec changes needed: `--last-id` 文档不能再写成“起点”，应写成 `max(cursor_start_after_id, last_id)` 的 floor，且不能回退 consumer progress。
- Tests or reviews that caught issues: `cargo test -p wiki-cli cursor -- --nocapture` 覆盖 cursor、fresh consumer、manual floor 三种导出路径。
- Next plan note: 下一项进入 `Multi-process Write Lock / Lease`，先处理多进程写入互斥，再扩大 reliability test matrix。

## 2026-04-28 / Multi-process Writer Lease

- Scope: 增加 `wiki.db.writer.lock` writer lease，写入型 CLI / MCP 入口在加载 engine 前拿锁；读命令不拿锁。
- What worked: 文件 `create_new` 能在不引新依赖、不改 DB schema 的情况下提供跨进程 fail-fast；TTL 保留崩溃恢复路径。
- What caused rework: CLI `main()` 返回 boxed error 时会打印 Rust variant；lease acquire 需要转成 Display 字符串，让 stderr 对操作员可读。
- Spec changes needed: writer 分类要明确 query/lint/gap 也属于 DB writer，因为它们会记录 query/lint/gap 运行状态或保存 snapshot。
- Tests or reviews that caught issues: storage tests 覆盖 busy/release/stale/owner-safe drop；CLI tests 覆盖 writer/read-only 分类、busy fail-fast、metrics 被锁时仍可读。
- Next plan note: 下一项是 `Reliability Test Matrix`，可以直接把 writer lease busy、DB lock/failure injection 和 MCP malformed/oversized 放进回归矩阵。

## 2026-04-28 / Reliability Test Matrix

- Scope: 只补测试，不改功能：DB batch rollback failure injection、大 snapshot smoke、MCP malformed JSON、LLM malformed JSON slice。
- What worked: SQLite trigger 比破坏 DB 文件更稳，能精准模拟 batch 中途失败并验证 rollback。
- What caused rework: MCP parse error 原先只在 `run_mcp` loop 里手写 JSON，无法直接单测；抽成 `parse_json_rpc_request_line()` 后行为不变但可覆盖。
- Spec changes needed: quick 矩阵和慢速 fuzz/perf 要拆开；本 PR 只放能进日常 CI 的确定性测试。
- Tests or reviews that caught issues: focused `cargo test -p wiki-storage reliability` 和 `cargo test -p wiki-cli reliability` 覆盖新增路径；workspace test 验证大 snapshot smoke 没拖垮 quick。
- Next plan note: 下一项进入 `Dependency Audit Automation`，保持低频 job，不塞进 quick 必跑路径。

## 2026-04-28 / Dependency Audit Automation

- Scope: 新增 scheduled/manual supply-chain audit lane；quick CI 只做脚本语法检查，不安装或运行 `cargo-audit`。
- What worked: 用独立 workflow 承载慢速外部 advisory DB 检查，避免 PR 必跑路径被网络/安装时间污染。
- What caused rework: 无。
- Spec changes needed: audit artifacts 只上传 GitHub Actions artifact；生产 Vault 报告接入留给 `Scheduled Vault Reports`。
- Tests or reviews that caught issues: `bash -n` 覆盖 wrapper 语法；workspace gate 确认 workflow/docs 改动不影响 Rust crates。
- Next plan note: 下一项进入 `Scheduled Vault Reports`，把应用层报告做定时生成、latest 指针和保留策略。

## 2026-04-28 / Scheduled Vault Reports

- Scope: 新增 automation `vault-reports` job，产出 timestamped report bundle、latest 指针和保留策略；不改变既有报告格式。
- What worked: 复用现有 scanner/render 函数，避免把 reporting 逻辑复制成第二套。
- What caused rework: daily plan 的稳定顺序测试需要同步补 `vault-reports`。
- Spec changes needed: latest 指针采用 JSON/Markdown 文件，不用 symlink，便于跨平台和 Vault 可见。
- Tests or reviews that caught issues: CLI integration test 直接跑 `automation run vault-reports`，验证 bundle 内所有文件存在；unit test 覆盖 retention pruning。
- Next plan note: 下一项进入 `Notion Archived Source Retirement audit/plan`，保持 DB-first，先 dry-run 不改 DB/Vault。

## 2026-04-28 / M12 Executor Dry-Run Planner

- Scope: 在 `wiki-cli suggest` 后增加 opt-in `--executor-plan`，从 `StrategyReport` 派生 typed dry-run action plan，不执行写入。
- What worked: 把 plan 作为独立模型和 report sibling，而不是改写现有 `StrategyReport`，保持 M12 首版 JSON 合同不漂移。
- What caused rework: 无。
- Spec changes needed: `command_preview` 只能做审计展示；下一项 guarded apply 必须使用 `action_kind` + allowlist，不能 shell parse。
- Tests or reviews that caught issues: 本 PR 增加 core plan 序列化/映射测试，以及 CLI JSON envelope/report-dir sibling 测试。
- Next plan note: 最后一项进入 M12 executor guarded apply：dry-run-first、allowlist、explicit apply flag。

## 2026-04-28 / M12 Executor Guarded Apply

- Scope: 新增 `wiki-cli suggest-executor-apply`，消费 `*-executor-plan.json`，默认 preflight；只有 `--apply` + `--allow fix-auto-safe` 执行低风险 auto fix。
- What worked: apply 不解析 `command_preview`，而是用 typed `action_kind`、`subject`、`suggestion_reason` 去匹配当前 auto fix，避免执行过期或伪造命令。
- What caused rework: dry-run plan 需要补 `suggestion_reason` 作为 evidence；旧 plan 缺该字段时必须重新生成。
- Spec changes needed: M12 executor 后续若增加 supersede/crystallize，必须先扩展 typed action 和 allowlist，不能直接复用 shell command。
- Tests or reviews that caught issues: suggest 集成测试覆盖 preflight 不写、`--apply` 无 allowlist 失败、allowlisted auto fix 成功 apply。
- Next plan note: 这项合并后 roadmap 23 项全部完成；剩余工作转入生产手动验收或新 roadmap。

## 2026-04-30 / Next Three Closeout

- Scope: 清掉 stale `vault-report-paths` 状态，增加只读 row-state 生产验证命令，观察 hardening schedule 首跑。
- What worked: `verify-row-state` 走 no-engine dispatch + read-only SQLite handle，能验证真实生产 DB 而不触发 writer lease、snapshot save、outbox 或 Vault projection。
- What caused rework: 生产 `wiki.db` 仍是 legacy blob-only 状态；验证命令必须把缺失 `wiki_state_row` 表转成可读失败，而不是 raw SQLite error。
- Spec changes needed: row-state blob fallback 退役必须拆成下一轮受控 migration/backfill，不能把“代码支持 row-level”误当成“生产已可退 blob”。
- Tests or reviews that caught issues: focused CLI tests 覆盖 matching rows、JSON、missing rows、legacy blob-only DB；生产只读验证返回 `rows=0 blob_present=true matches_blob=n/a`。
- Next plan note: Hardening scheduled run `25150878853` 已 green；除非未来 schedule 失败，否则不需要修复 PR。
