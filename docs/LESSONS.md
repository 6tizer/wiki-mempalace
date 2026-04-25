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
