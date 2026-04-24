# Automation Health / Alert Strategy

本文件定义 `wiki-cli automation health` 的当前运维判定规则。

## 命令入口

- `cargo run -p wiki-cli -- --db wiki.db automation last-failures --limit 10`
- `cargo run -p wiki-cli -- --db wiki.db automation health`
- `cargo run -p wiki-cli -- --db wiki.db automation health --summary-file wiki/reports/automation-health.txt`

## 健康级别

- `green`：当前没有触发阈值，默认无需人工介入
- `yellow`：存在需要关注的问题，但未达到阻断级别
- `red`：存在明显异常，应该在下一次 `run-daily` 前介入处理

## 当前阈值

- 心跳超时：
  - `yellow`：运行中 job 的 `heartbeat_at` 超过 6 小时未更新
  - `red`：运行中 job 的 `heartbeat_at` 超过 24 小时未更新
- 连续失败：
  - `yellow`：同一 job 连续失败达到 2 次
  - `red`：同一 job 连续失败达到 3 次
- consumer backlog：
  - `yellow`：`backlog_events >= 25`
  - `red`：`backlog_events >= 100`

## 输出与退出码

- `automation last-failures`：
  - 只列最近失败记录，便于直接定位错误摘要
- `automation health`：
  - stdout 输出完整健康摘要，适合人读和文件落地
  - 若 `--summary-file` 提供路径，会把同一份摘要写入本地文件
  - 若总体状态为 `yellow`，stderr 输出 `ALERT YELLOW`
  - 若总体状态为 `red`，stderr 输出 `ALERT RED`，并以退出码 `1` 结束

## 介入建议

- `stale-heartbeat`：
  - 检查对应 job 是否卡死、是否存在外部依赖超时、是否需要重新触发
- `consecutive-failures`：
  - 先看 `automation last-failures` 的错误摘要，再看对应 job 的输入和最近改动
- `consumer-backlog`：
  - 检查 `consume-to-mempalace` 是否长期未运行，或 outbox 生产速度是否异常升高

## 当前边界

- 这套规则只覆盖本地最小运维面，不直接连外部通知系统
- 告警摘要当前是纯文本，后续批次如需接监控平台，可在此基础上再加结构化输出
