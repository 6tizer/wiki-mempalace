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
