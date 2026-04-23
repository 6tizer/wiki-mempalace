#!/usr/bin/env bash
# 恢复演练脚本。
# 目标：把备份包恢复到 scratch 目录，实际执行 palace.db 重建，再用
# `wiki-cli automation verify-restore` 做统一恢复成功检查。脚本不直接改生产路径。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BACKUP_DB=""
BACKUP_WIKI_TAR=""
SCRATCH=""
SKIP_PALACE_REBUILD=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db)
      BACKUP_DB="${2:-}"
      shift 2
      ;;
    --wiki-tar)
      BACKUP_WIKI_TAR="${2:-}"
      shift 2
      ;;
    --scratch)
      SCRATCH="${2:-}"
      shift 2
      ;;
    --skip-palace-rebuild)
      SKIP_PALACE_REBUILD=1
      shift
      ;;
    -h|--help)
      cat <<'EOF'
用法：
  bash scripts/recovery-drill.sh --db <backup.db> --wiki-tar <backup.tar.gz> [--scratch <dir>] [--skip-palace-rebuild]

作用：
  1. 把备份 db 复制到 scratch/wiki.db
  2. 校验 sqlite integrity_check
  3. 解包 vault 并检查 pages/index/log/sources
  4. 默认实际重建 scratch/palace.db
  5. 调用 `automation verify-restore` 做统一恢复成功检查
EOF
      exit 0
      ;;
    *)
      echo "未知参数: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$BACKUP_DB" ]]; then
  echo "--db 是必填参数" >&2
  exit 2
fi
if [[ ! -f "$BACKUP_DB" ]]; then
  echo "备份 db 不存在: $BACKUP_DB" >&2
  exit 1
fi
if [[ -z "$BACKUP_WIKI_TAR" ]]; then
  echo "--wiki-tar 是必填参数；缺 vault 视为恢复不完整" >&2
  exit 2
fi
if [[ ! -f "$BACKUP_WIKI_TAR" ]]; then
  echo "备份 tar 不存在: $BACKUP_WIKI_TAR" >&2
  exit 1
fi

if [[ -z "$SCRATCH" ]]; then
  SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/wiki-recovery.XXXXXX")"
else
  mkdir -p "$SCRATCH"
fi

RESTORED_DB="$SCRATCH/wiki.db"
cp "$BACKUP_DB" "$RESTORED_DB"

check="$(sqlite3 "$RESTORED_DB" 'PRAGMA integrity_check;' | head -n 1)"
if [[ "$check" != "ok" ]]; then
  echo "wiki.db integrity_check 失败: $check" >&2
  exit 1
fi
echo "PRAGMA integrity_check=ok"

outbox_max_id="$(sqlite3 "$RESTORED_DB" 'SELECT COALESCE(MAX(id), 0) FROM wiki_outbox;' | head -n 1)"

echo "RESTORED_DB=$RESTORED_DB"
echo "OUTBOX_MAX_ID=$outbox_max_id"

tar -xzf "$BACKUP_WIKI_TAR" -C "$SCRATCH"

RESTORED_WIKI="$SCRATCH/wiki"
if [[ ! -d "$RESTORED_WIKI" ]]; then
  candidate="$(find "$SCRATCH" -mindepth 1 -maxdepth 1 -type d | head -n 1 || true)"
  RESTORED_WIKI="${candidate:-$SCRATCH}"
fi

if [[ ! -f "$RESTORED_WIKI/index.md" || ! -f "$RESTORED_WIKI/log.md" ]]; then
  echo "vault 缺少 index.md 或 log.md: $RESTORED_WIKI" >&2
  exit 1
fi
if [[ ! -d "$RESTORED_WIKI/pages" ]]; then
  echo "vault 缺少 pages/ 目录: $RESTORED_WIKI" >&2
  exit 1
fi
if [[ ! -d "$RESTORED_WIKI/sources" ]]; then
  echo "vault 缺少 sources/ 目录: $RESTORED_WIKI" >&2
  exit 1
fi

checked=0
while IFS= read -r -d '' f; do
  first_line="$(head -n 1 "$f")"
  if [[ "$first_line" != "---" ]]; then
    echo "frontmatter missing in $f" >&2
    exit 1
  fi
  grep -q "^status:" "$f" || { echo "status field missing in $f" >&2; exit 1; }
  checked=$((checked + 1))
done < <(find "$RESTORED_WIKI/pages" -type f -name '*.md' -print0)

if [[ "$checked" -eq 0 ]]; then
  echo "vault frontmatter 检查失败：pages/ 下没有 md 文件" >&2
  exit 1
fi

echo "RESTORED_WIKI=$RESTORED_WIKI"
echo "PAGES_FRONTMATTER_OK=$checked"
echo "VOLUME_OK=pages,sources,index,log"

RESTORED_PALACE="$SCRATCH/palace.db"
if [[ "$SKIP_PALACE_REBUILD" -eq 0 ]]; then
  rm -f "$RESTORED_PALACE"
  (
    cd "$REPO_ROOT"
    cargo run -q -p wiki-cli -- \
      --db "$RESTORED_DB" \
      --wiki-dir "$RESTORED_WIKI" \
      --palace "$RESTORED_PALACE" \
      consume-to-mempalace --last-id 0
  )
  (
    cd "$REPO_ROOT"
    cargo run -q -p wiki-cli -- \
      --db "$RESTORED_DB" \
      --wiki-dir "$RESTORED_WIKI" \
      --palace "$RESTORED_PALACE" \
      automation verify-restore
  )
  echo "RESTORED_PALACE=$RESTORED_PALACE"
else
  (
    cd "$REPO_ROOT"
    cargo run -q -p wiki-cli -- \
      --db "$RESTORED_DB" \
      --wiki-dir "$RESTORED_WIKI" \
      automation verify-restore
  )
fi

echo "RECOVERY_DRILL_OK=$SCRATCH"
