# LongMemEval 与 CI 策略

[LongMemEval](https://arxiv.org/abs/2402.17753) 是用于评估对话式长期记忆系统的公开基准之一。与本仓库内置的 `bench`（基于本地 palace 语料自采样、度量 recall@k 与延迟）不同，LongMemEval 通常要求：

- **固定评测协议与数据分发**：需遵守原作者 / 托管方的许可与引用要求；
- **较重的依赖或模型调用**：部分设置依赖特定 API 或较大模型权重；
- **稳定的 golden 集与打分脚本**：接入 CI 意味着下载数据、缓存与可复现的运行环境。

**当前结论（实现计划阶段）：**

- 默认 **不将 LongMemEval 全量纳入** `.github/workflows` 的必跑门禁，以避免 CI 体积、网络与许可不确定性拖慢 PR。
- 推荐做法：在本地或独立 workflow（`workflow_dispatch`）中，在确认许可后手动运行上游评测脚本，将结果以 artifact 或报告形式归档；本仓库的 `bench --mode fixed` 继续承担**回归对比**职责。
- 若未来引入「LongMemEval 子集」fixture，应使用明确允许再分发的子样本，并在本文件中更新许可链接与版本钉扎信息。

相关外部参考：

- Hindsight 与 LongMemEval 的公开论述见其 [README](https://github.com/vectorize-io/hindsight) 与文档站点。
- 本仓库检索与 bench 实现见 `src/service.rs` 中 `search_with_options`、`benchmark_run`。