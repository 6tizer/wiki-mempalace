# Recovery Drill Record Template

## 基本信息

- 演练日期：
- 操作人：
- 关联分支 / commit：
- 备份来源：
  - `wiki.db`：
  - `wiki.tar.gz`：
- scratch 路径：

## 执行命令

```bash
bash scripts/recovery-drill.sh \
  --db <backup.db> \
  --wiki-tar <backup.tar.gz> \
  --scratch <scratch-dir>
```

## `wiki.db` 校验结果

- `PRAGMA integrity_check`：
- outbox `head_id`：
- outbox `total_events`：
- 备注：

## vault 校验结果

- `index.md`：
- `log.md`：
- `pages/`：
- `sources/`：
- `frontmatter_checked`：
- 备注：

## `palace.db` 重建结果

- 是否执行重建：
- `drawers`：
- `kg_facts`：
- `acked_up_to_id`：
- `backlog_events`：
- 备注：

## 异常与处理

- 异常 1：
- 处理：
- 是否需要补文档 / 补自动化：

## 后续动作

- [ ] 回填 runbook / 计划文档
- [ ] 生成新的演练基线
- [ ] 触发下一次定期演练
