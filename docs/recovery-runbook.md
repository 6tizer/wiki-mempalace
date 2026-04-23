# P0 / M5 恢复与回滚 Runbook

本文是 `wiki-mempalace` 的最小可执行恢复方案。目标不是做自动修复器，而是把事故后的判断、恢复、重建和验证收敛成一套可重复执行的操作。

## 恢复边界

先把三个资产分清：

| 资产 | 首选恢复方式 | 是否可从 outbox 重建 | 备注 |
| --- | --- | --- | --- |
| `wiki.db` | 从 `scripts/backup.sh` 产出的 `.db` 热备恢复 | 否 | 主库，所有结构化状态都以它为准 |
| `palace.db` | 优先从 outbox 重建；只有在你有同一时间点的 palace 快照时才直接恢复 | 是 | 下游记忆层，默认视为派生数据 |
| vault 文件 | 从 `scripts/backup.sh` 产出的 `.tar.gz` 恢复整个 `wiki/` 目录 | 否 | `sources/` 不能靠 outbox 补回 |

`wiki.db-wal` 和 `wiki.db-shm` 是运行时临时文件，不是备份资产。恢复时应在停写后删除它们，避免把半写入状态带回去。

## 运行顺序

恢复顺序固定为：

1. 停止所有会写 `wiki.db` 或 `wiki/` 的任务。
2. 先恢复 `wiki.db`。
3. 再恢复 vault 文件。
4. 最后按 outbox 重建 `palace.db`。
5. 做完整性和投影验证。

不要反过来做。`palace.db` 依赖 `wiki.db` 的 outbox 状态；vault 文件依赖 `wiki.db` 对应的投影结果和备份包。

## 1. 从备份恢复 `wiki.db`

推荐做法是直接恢复热备文件，而不是尝试手工拼回 WAL。

```bash
# 停止写入后，先保留当前故障现场
mv wiki.db "wiki.db.broken.$(date +%Y%m%d-%H%M%S)" 2>/dev/null || true
rm -f wiki.db-wal wiki.db-shm

# 把备份库放回原位
cp /path/to/backups/wiki-YYYYmmdd-HHMMSS.db wiki.db

# 完整性校验
sqlite3 wiki.db 'PRAGMA integrity_check;'
```

如果 `integrity_check` 不是 `ok`，这份备份不要继续用。换上一份更早的已知良好备份。

## 2. `palace.db` 的恢复 / 重建策略

默认策略是“**从 outbox 重建**”，因为 `palace.db` 是派生层，不应该作为唯一真相来源。

### 推荐路径：从 outbox 重建

对一个新建或已损坏的 `palace.db`，最稳妥的路径是从恢复后的 `wiki.db` 重新消费 outbox：

```bash
# 全量重建时，通常从 0 开始回放恢复后的 snapshot
cargo run -p wiki-cli -- \
  --db wiki.db \
  consume-to-mempalace --last-id 0
```

如果你有可靠且已记录的消费游标，并且能确认它不晚于这次恢复的 `wiki.db` 快照，可以把 `0` 换成那个游标；否则就从 `0` 全量回放。

如果你使用的是显式导出 + 外部 consumer，则顺序是：

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  export-outbox-ndjson-from --last-id 0 > events.ndjson

# 由你的 consumer 处理 events.ndjson 后，再按消费进度 ack
cargo run -p wiki-cli -- \
  --db wiki.db \
  ack-outbox --up-to-id <high-water-mark> --consumer-tag mempalace-rebuild
```

### 只在一种情况下直接恢复

如果你手头有一份**与同一 `wiki.db` 快照配套**的 `palace.db` 备份，可以直接恢复它。前提是你确认它和 `wiki.db` 是同一时间点的配对产物。

否则不要把更晚的 `palace.db` 和更早的 `wiki.db` 混着用。那样只会制造更隐蔽的不一致。

## 3. vault 文件恢复

vault 的完整恢复只能来自 `scripts/backup.sh` 产生的 `.tar.gz`，因为当前投影层只保证 `pages/`、`index.md`、`log.md`，不会替你重建 `sources/` 根内容。

```bash
# 恢复前先把旧 vault 挪走
mv wiki "wiki.broken.$(date +%Y%m%d-%H%M%S)" 2>/dev/null || true

# 解包整个 vault 目录
tar -xzf /path/to/backups/wiki-YYYYmmdd-HHMMSS.tar.gz -C /path/to/parent
```

恢复后至少要检查：

```bash
test -f wiki/index.md
test -f wiki/log.md
test -d wiki/pages
test -d wiki/sources
```

如果只有 `wiki.db`，而没有 vault tarball，那么你只能把结构化状态恢复回来，不能声称 vault 完整恢复。`sources/` 必须从 tarball 或上游导出重新拿回。

## 4. 演练步骤

演练原则：**永远先在 scratch 目录做一次完整恢复**，不要拿生产路径直接试。

推荐的最小演练顺序：

1. 先运行一次热备：
   ```bash
   bash scripts/backup.sh --db wiki.db --wiki wiki --out /tmp/wiki-backups
   ```
2. 用备份包在临时目录恢复：
   ```bash
   bash scripts/recovery-drill.sh \
     --db /tmp/wiki-backups/wiki-YYYYmmdd-HHMMSS.db \
     --wiki-tar /tmp/wiki-backups/wiki-YYYYmmdd-HHMMSS.tar.gz
   ```
3. 确认脚本输出 `PRAGMA integrity_check=ok`、frontmatter 扫描通过、`sources/` 和 `pages/` 都在。
4. 脚本会默认在 scratch 目录实际执行：
   - `consume-to-mempalace --last-id 0`
   - `automation verify-restore`
5. 确认输出里出现：
   - `RESTORED_DB=...`
   - `RESTORED_WIKI=...`
   - `RESTORED_PALACE=...`
   - `RECOVERY_DRILL_OK=...`

## 5. 验证步骤

恢复完成后，统一入口是：

```bash
cargo run -p wiki-cli -- \
  --db wiki.db \
  --wiki-dir wiki \
  --palace /path/to/palace.db \
  automation verify-restore
```

它会一次性验证：

- `wiki.db integrity_check`
- snapshot / outbox 可读
- vault 的 `index.md` / `log.md` / `pages/` / `sources/`
- `pages/` frontmatter 和 `status:`
- 可选 `palace.db` 的核心表与计数
- 可选 `mempalace` consumer progress / backlog

若你只想做最小 SQL / 文件级 spot check，仍可手工跑下面这些：

```bash
sqlite3 wiki.db 'PRAGMA integrity_check;'
sqlite3 wiki.db 'SELECT COUNT(*) FROM wiki_outbox;'
test -f wiki/index.md
test -f wiki/log.md
```

## 6. 回滚判断

出现以下任一情况，就直接回滚到上一份已知良好快照：

- `wiki.db` 的 `integrity_check` 失败
- 恢复后的 vault 缺少 `sources/` 或 `pages/`
- outbox 重建后 `palace.db` 的读侧结果和预期不符
- 你无法确认 `wiki.db` 和 `palace.db` 是否来自同一时间点

回滚时优先保持“同一批备份一起恢复”，不要把新旧两套快照拼接。
