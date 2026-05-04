# Module Handovers

本目录存放 subagent 模块交接文档。

命名建议：

- `docs/handovers/<feature>/<module>.md`

规则：

- 每个复杂模块完成后写一份 handoff。
- handoff 必须能让新窗口只凭 spec 片段和本文档接手。
- 合并后可保留为历史，也可在对应 spec 归档时一起归档。

模板见 [../templates/module-handoff.md](../templates/module-handoff.md)。

## Current Handovers

- [notion-full-reimport-20260505/summary.md](notion-full-reimport-20260505/summary.md) — Full Notion three-DB reimport branch handoff and staging/production replacement checklist.
- [notion-source-vault-projection/summary.md](notion-source-vault-projection/summary.md) — PR #42 merge and production apply closeout; next workflow is Notion source compilation.
- [production-wiki-compiler/summary.md](production-wiki-compiler/summary.md) — Notion-equivalent local Wiki Compiler implementation and production tiny-sample handoff.
