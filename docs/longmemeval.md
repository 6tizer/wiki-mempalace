# LongMemEval 评测策略

[LongMemEval](https://arxiv.org/abs/2402.17753) 是用于评估对话式长期记忆系统的公开基准之一。与本仓库内置的 `bench`（基于本地 palace 语料自采样、度量 recall@k 与延迟）不同，LongMemEval 通常要求：

- **固定评测协议与数据分发**：需遵守原作者 / 托管方的许可与引用要求；
- **较重的依赖或模型调用**：部分设置依赖特定 API 或较大模型权重；
- **稳定的 golden 集与打分脚本**：接入 CI 意味着下载数据、缓存与可复现的运行环境。

## J13：本地检索基线

J13 的目标很窄：拿标准题考 `rust-mempalace` 本地检索能力，看它能不能把正确记忆找出来，并排在前面。

它不回答问题，不做 LLM judge，不自动修检索，也不接外部 embedding。它只出成绩单和错题本。

J13 计划：

- nightly sample：每天北京时间 03:00 跑 50 题。
- weekly full：每周日北京时间 04:00 跑全量。
- manual：保留 `workflow_dispatch`，方便手动复跑。
- artifact：保留 30 天。

核心指标：

- `R@1`：第一条结果是否找对。
- `R@5`：前五条里是否有正确记忆。
- `MRR`：正确记忆排得越靠前分越高。
- failed cases：错题本。
- `total_runtime_sec`、`avg_query_ms`、`throughput_per_sec`、`timeout_count`：运行健康。

分数低只写进报告，不让 workflow 失败。脚本崩、数据坏、超时、报告缺失才让 workflow 失败。

## 为什么不进 PR 必跑 CI

LongMemEval 是阶段性评测，不是每个 PR 都要跑的小检查。

它可能要联网下载数据，可能受上游 schema 或许可变化影响，full run 也可能比较慢。把它放进 required PR CI，会让普通代码改动被外部网络、数据漂移或评测耗时拖住。

PR 仍跑仓库常规检查。LongMemEval 走 scheduled / manual workflow，上传 artifact，用于观察趋势。

## J14：语义融合评测

J14 已作为 J13 runner 的扩展实现：使用 `--compare-semantic-fusion` 时，
每个 case 会复用同一份 mined palace 数据，分别跑：

- `query_baseline`：关闭 vector / RRF 权重的 query baseline。
- `semantic_fusion`：当前 `rust-mempalace` 默认 sparse semantic + RRF 融合检索。

报告保持原有顶层 `metrics` / `runtime` / `failed_cases` 字段，并新增：

- `comparison_enabled`
- `primary_variant`
- `metrics_by_variant`
- 每个 case 的 `variant_results`

scheduled / manual LongMemEval workflow 默认启用比较。低分只进 artifact，不让
workflow 失败；脚本崩、数据坏、超时、报告缺失仍然 fail。

外部 embedding / LLM rerank 仍未进入默认 J14。那条线会引入 key、费用、限流和
缓存，后续要另开 PRD/spec。

## 数据与许可

相关外部参考：

- Hindsight 与 LongMemEval 的公开论述见其 [README](https://github.com/vectorize-io/hindsight) 与文档站点。
- 本仓库检索与 bench 实现见 `crates/rust-mempalace/src/service.rs` 中 `search_with_options`、`benchmark_run`。

若未来引入 LongMemEval 子集 fixture，应使用明确允许再分发的子样本，并在本文件中更新许可链接与版本钉扎信息。完整数据只进入 `.cache/longmemeval/`，不提交到仓库。
