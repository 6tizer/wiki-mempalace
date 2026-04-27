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
- Next plan note: 系统开发闭环已完成。下一步不是再补 compiler 架构，而是继续小批量跑剩余 132 条 source；每批固定执行 deferred apply、Mempalace consume、lint/audit、重复页检查和 Vault spot-check。

## 2026-04-27 / Compiler Model Candidate Trial

- Scope: 用临时 DB/Vault 对比 compiler 模型，不触碰真实 `/Users/mac-mini/Documents/wiki`。
- What worked: `deepseek/deepseek-v4-flash` 在 OpenRouter 小 JSON smoke 1.83s；同一 `Avatar V` full compiler 成功 117.0s；再跑 3 条 X source 全部成功，单条 89.5-99.1s。
- What caused rework: 速度和 JSON 稳定性够用，但仍会产生归一化质量问题：重复近义概念、主实体 deferred、人名 Unicode 标题异常。
- Spec changes needed: 模型替换不能只看成功率和速度，compiler temp-vault regression 要加入“主实体页存在、近义 concept 不重复、人名/文件名 Unicode 正常化”验收项。
- Tests or reviews that caught issues: 临时 3-source smoke 抓到 `自愈式浏览器自动化` / `自愈浏览器自动化` 重复，以及 `Magnus M ü ller` 标题异常；`compiler-resolve-deferred` dry-run 仍保持低置信 deferred。
- Next plan note: 记录 `deepseek/deepseek-v4-flash` 为待选模型；生产默认暂不切换，先补 resolver/fixer 归一化能力。
