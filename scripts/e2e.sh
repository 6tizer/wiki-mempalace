#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKDIR="${TMPDIR:-/tmp}/llm-wiki-e2e"

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
cd "$WORKDIR"

echo "[1/8] ingest + projection"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --sync-wiki \
  ingest "file:///tmp/a.md" "项目使用 Redis"$'\n'"Authorization: Bearer secret" --scope private:cli

echo "[2/8] file claim"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db file-claim "项目使用 Redis" --scope private:cli --tier semantic

echo "[3/8] supersede claim"
old_id="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db file-claim "v1" --scope private:cli --tier semantic | sed -n 's/^claim_id=//p')"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db supersede-claim "$old_id" "v2" --scope private:cli --tier semantic

echo "[4/8] query with write-page"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --sync-wiki \
  query "Redis API" --write-page --page-title "analysis-redis-api"

echo "[5/8] lint + report"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --sync-wiki lint

echo "[5.1] frontmatter check"
# pages/ 下每个 .md 的第一行必须是 ---（YAML frontmatter 存在）
for f in wiki/pages/*.md; do
  [[ -f "$f" ]] || continue
  first_line="$(head -n 1 "$f")"
  if [[ "$first_line" != "---" ]]; then
    echo "frontmatter missing in $f (first line: $first_line)" >&2
    exit 1
  fi
  grep -q "^status:" "$f" || { echo "status field missing in $f" >&2; exit 1; }
done
echo "  pages frontmatter OK"

echo "[6/8] outbox export + ack"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db export-outbox-ndjson-from --last-id 0 | sed -n '1,5p'
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db ack-outbox --up-to-id 999999 --consumer-tag e2e

echo "[7/8] mempalace consume"
consume_output="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db consume-to-mempalace --last-id 0)"
echo "$consume_output"
consumed_count="$(echo "$consume_output" | sed -n 's/^consumed=//p' | tail -n 1)"
if [[ -z "$consumed_count" || "$consumed_count" -le 0 ]]; then
  echo "Expected consumed>0, got: ${consumed_count:-empty}" >&2
  exit 1
fi

echo "[8/8] llm smoke (optional)"
if [[ -f "$REPO_ROOT/llm-config.toml" ]]; then
  llm_out="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
    llm-smoke --config "$REPO_ROOT/llm-config.toml" --prompt "Say 'ok' only.")"
  echo "$llm_out"
else
  echo "skip: llm-config.toml not found"
fi

echo "[9] viewer-scope isolation (no ranked lines for wrong private agent)"
ranked_wrong="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --viewer-scope private:intruder query "Redis" 2>/dev/null | grep -E '^[0-9]' || true)"
if [[ -n "$ranked_wrong" ]]; then
  echo "expected empty ranked results for private:intruder, got: $ranked_wrong" >&2
  exit 1
fi

test -f wiki/index.md
test -f wiki/log.md
test -d wiki/reports

echo "[10] backup smoke"
BACKUP_OUT="$WORKDIR/backups"
out="$("$REPO_ROOT/scripts/backup.sh" --db "$WORKDIR/wiki.db" --wiki "$WORKDIR/wiki" --out "$BACKUP_OUT")"
echo "$out"
backup_db="$(echo "$out" | sed -n 's/^BACKUP_DB=//p')"
if [[ -z "$backup_db" || ! -f "$backup_db" ]]; then
  echo "backup 未生成数据库文件: ${backup_db:-empty}" >&2
  exit 1
fi
# 备份库必须能被 sqlite3 打开，且包含业务核心表 wiki_state / wiki_outbox
tables="$(sqlite3 "$backup_db" "SELECT name FROM sqlite_master WHERE type='table';")"
for required in wiki_state wiki_outbox; do
  if ! echo "$tables" | grep -q "^${required}$"; then
    echo "备份库缺少 ${required} 表，实际 tables: $tables" >&2
    exit 1
  fi
done
# wiki 目录打包文件应存在
ls "$BACKUP_OUT"/wiki-*.tar.gz >/dev/null
echo "  backup smoke OK"

echo "E2E PASS: $WORKDIR"
