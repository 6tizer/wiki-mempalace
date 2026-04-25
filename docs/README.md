# Documentation Index

本目录只保留当前事实、当前计划和少量仍有运行价值的参考文档。已完成的批次计划和旧路线图已移到 [archive/](archive/README.md)。

## 当前事实

- [architecture.md](architecture.md) — crate 拓扑、ingest/query/outbox/MCP 业务流。
- [mempalace-linkage.md](mempalace-linkage.md) — wiki 与 rust-mempalace 的 bridge 契约。
- [outbox-and-consumers.md](outbox-and-consumers.md) — outbox 表、per-consumer progress、ack 语义。
- [outbox-event-matrix.md](outbox-event-matrix.md) — `WikiEvent` 生产者、消费者和测试覆盖。
- [vault-standards.md](vault-standards.md) — vault 目录、命名、frontmatter、正文骨架唯一标准。

## 运行与恢复

- [automation-health-alerts.md](automation-health-alerts.md) — `wiki-cli automation health` 阈值、退出码、介入建议。
- [recovery-runbook.md](recovery-runbook.md) — `wiki.db`、vault、`palace.db` 恢复流程。
- [recovery-drill-template.md](recovery-drill-template.md) — 恢复演练记录模板。

## 活跃计划

- [roadmap.md](roadmap.md) — M1-M9 已完成；M10-M12 是当前下一阶段。
- [automation-issue-batch-3.md](automation-issue-batch-3.md) — M10 metrics、M11 dashboard、M12 strategy、Schema T2 tags、LongMemEval auto benchmark 的批次计划。
- [dev-workflow.md](dev-workflow.md) — PRD → spec 三件套 → branch → subagent → review → PR → CI → merge 的固定开发流程。
- [LESSONS.md](LESSONS.md) — 每轮合并后的项目级经验，下一轮 Plan mode 前必读。
- [prd/batch-3.md](prd/batch-3.md) — batch-3 PRD。
- [specs/](specs/) — batch-3 各模块 spec 三件套。
- [handovers/](handovers/) — subagent 模块交接文档。
- [schema-followup-plan.md](schema-followup-plan.md) — T0/T1 已完成；T2/T3 标签治理与延后项仍可继续。
- [longmemeval.md](longmemeval.md) — LongMemEval / CI 策略。

## 模板

- [templates/prd.md](templates/prd.md)
- [templates/spec-requirements.md](templates/spec-requirements.md)
- [templates/spec-design.md](templates/spec-design.md)
- [templates/spec-tasks.md](templates/spec-tasks.md)
- [templates/subagent-task.md](templates/subagent-task.md)
- [templates/module-handoff.md](templates/module-handoff.md)
- [templates/review-checklist.md](templates/review-checklist.md)

## 历史资料

- [archive/](archive/README.md) — 已完成或被当前 roadmap 取代的计划与清单。
- [blog/article2.md](blog/article2.md) — 两仓合并前后的历史长文，不作为当前实现事实源。
