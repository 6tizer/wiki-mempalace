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
