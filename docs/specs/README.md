# Spec Index

本目录存放每个功能模块的 spec 三件套：

- `requirements.md` — 行为、输入输出、验收标准。
- `design.md` — 数据结构、接口、流程、兼容性。
- `tasks.md` — task 分级、状态、review、验证。

规则：

- spec 是实现源。代码和 spec 不一致时，先改 spec，再改代码。
- 每个模块独立维护三件套。
- 模块完成后更新 tasks 状态和 checklist。

## Active Specs

- [m10-metrics/](m10-metrics/) — M10 unified metrics core. Merged PR #12。
- [m11-dashboard/](m11-dashboard/) — M11 read-only dashboard/report. Merged PR #14。
- [m12-strategy/](m12-strategy/) — M12 strategy suggestions. Merged PR #16。
- [schema-t2-tags/](schema-t2-tags/) — Schema T2 tag governance. Merged PR #13。
- [longmemeval-auto/](longmemeval-auto/) — LongMemEval scheduled benchmark artifacts. Planned。
