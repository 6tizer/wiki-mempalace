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
