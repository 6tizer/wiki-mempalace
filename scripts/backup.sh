#!/usr/bin/env bash
# D4：SQLite 热备份脚本。
# 用 `sqlite3 .backup` 做 online backup（支持 WAL 模式下不阻塞写入），
# 同时可选打包 wiki 投影目录，便于整体快照归档。
#
# 用法：
#   scripts/backup.sh [--db PATH] [--wiki PATH] [--out DIR]
#
# 默认：
#   --db   ~/wiki-mempalace/wiki.db
#   --wiki ~/wiki-mempalace/wiki
#   --out  ~/wiki-mempalace/backups
#
# 输出：<out>/wiki-YYYYmmdd-HHMMSS.db（+ 可选 .tar.gz）

set -euo pipefail

DB="${WIKI_DB:-$HOME/wiki-mempalace/wiki.db}"
WIKI_DIR="${WIKI_DIR:-$HOME/wiki-mempalace/wiki}"
OUT_DIR="${WIKI_BACKUP_DIR:-$HOME/wiki-mempalace/backups}"

# 解析参数
while [[ $# -gt 0 ]]; do
  case "$1" in
    --db)   DB="$2";       shift 2 ;;
    --wiki) WIKI_DIR="$2"; shift 2 ;;
    --out)  OUT_DIR="$2";  shift 2 ;;
    -h|--help)
      grep '^# ' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "未知参数: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! -f "$DB" ]]; then
  echo "数据库不存在: $DB" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
TS="$(date +%Y%m%d-%H%M%S)"
DB_OUT="$OUT_DIR/wiki-$TS.db"

# sqlite3 .backup 为在线热备，自动处理 WAL 检查点
echo "[backup] $DB → $DB_OUT"
sqlite3 "$DB" ".backup '$DB_OUT'"

# 简单完整性校验：备份文件的 integrity_check 必须为 ok
check="$(sqlite3 "$DB_OUT" 'PRAGMA integrity_check;' | head -n 1)"
if [[ "$check" != "ok" ]]; then
  echo "[backup] integrity_check 失败: $check" >&2
  exit 1
fi
echo "[backup] integrity_check=ok"

# 可选：若存在 wiki 投影目录，一并打包
if [[ -d "$WIKI_DIR" ]]; then
  TAR_OUT="$OUT_DIR/wiki-$TS.tar.gz"
  echo "[backup] wiki 目录打包 → $TAR_OUT"
  tar -czf "$TAR_OUT" -C "$(dirname "$WIKI_DIR")" "$(basename "$WIKI_DIR")"
fi

# 打印结果路径，方便脚本消费
echo "BACKUP_DB=$DB_OUT"
