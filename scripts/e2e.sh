#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKDIR="${TMPDIR:-/tmp}/llm-wiki-e2e"

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
cd "$WORKDIR"

echo "[1/11] ingest + projection"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --sync-wiki \
  ingest "file:///tmp/a.md" "项目使用 Redis"$'\n'"Authorization: Bearer secret" --scope private:cli

echo "[2/11] file claim"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db file-claim "项目使用 Redis" --scope private:cli --tier semantic

echo "[3/11] supersede claim"
old_id="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db file-claim "v1" --scope private:cli --tier semantic | sed -n 's/^claim_id=//p')"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db supersede-claim "$old_id" "v2" --scope private:cli --tier semantic

echo "[4/11] query with write-page"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --sync-wiki \
  query "Redis API" --write-page --page-title "analysis-redis-api"

echo "[5/11] lint + report"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --sync-wiki lint

echo "[5.1] frontmatter check"
# pages/<entry_type>/*.md 下每个 .md 的第一行必须是 ---（YAML frontmatter 存在）
# 现投影层按 entry_type 分子目录（见 docs/vault-standards.md），需递归扫描。
# 用 `find -print0 | while read -d ''` 保证可移植（macOS bash 3.x 无 mapfile）。
checked=0
while IFS= read -r -d '' f; do
  first_line="$(head -n 1 "$f")"
  if [[ "$first_line" != "---" ]]; then
    echo "frontmatter missing in $f (first line: $first_line)" >&2
    exit 1
  fi
  grep -q "^status:" "$f" || { echo "status field missing in $f" >&2; exit 1; }
  checked=$((checked + 1))
done < <(find wiki/pages -type f -name '*.md' -print0 2>/dev/null)
if [[ "$checked" -eq 0 ]]; then
  echo "frontmatter check: no page files found under wiki/pages/**, aborting" >&2
  exit 1
fi
echo "  pages frontmatter OK ($checked files checked)"

echo "[6/11] outbox export + ack"
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db export-outbox-ndjson-from --last-id 0 | sed -n '1,5p'
cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db ack-outbox --up-to-id 999999 --consumer-tag e2e

echo "[7/11] mempalace consume"
consume_output="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db consume-to-mempalace --last-id 0)"
echo "$consume_output"
consumed_count="$(echo "$consume_output" | sed -n 's/^consumed=//p' | tail -n 1)"
if [[ -z "$consumed_count" ]]; then
  consumed_count="$(echo "$consume_output" | sed -n 's/^seen=.*dispatched=\([0-9][0-9]*\).*$/\1/p' | tail -n 1)"
fi
if [[ -z "$consumed_count" || "$consumed_count" -le 0 ]]; then
  echo "Expected consumed/dispatched >0, got: ${consumed_count:-empty}" >&2
  exit 1
fi

echo "[8/11] llm smoke (optional)"
if [[ -f "$REPO_ROOT/llm-config.toml" ]]; then
  if llm_out="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
    llm-smoke --config "$REPO_ROOT/llm-config.toml" --prompt "Say 'ok' only." 2>/dev/null)"; then
    echo "$llm_out"
  else
    echo "skip: optional llm smoke failed"
  fi
else
  echo "skip: llm-config.toml not found"
fi

echo "[9/11] viewer-scope isolation (no ranked lines for wrong private agent)"
ranked_wrong="$(cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --viewer-scope private:intruder query "Redis" 2>/dev/null | grep -E '^[0-9]' || true)"
if [[ -n "$ranked_wrong" ]]; then
  echo "expected empty ranked results for private:intruder, got: $ranked_wrong" >&2
  exit 1
fi

test -f wiki/index.md
test -f wiki/log.md
test -d wiki/reports

echo "[10/11] backup smoke"
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

echo "[11/11] wiki-agent native/fallback/memory smoke"
agent_doctor="$(cargo run -q -p wiki-agent --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --viewer-scope private:cli --palace palace.db \
  doctor --tool-backend native)"
echo "$agent_doctor"
echo "$agent_doctor" | grep -q "^tools=22$" || {
  echo "wiki-agent native doctor did not discover 22 tools" >&2
  exit 1
}

mcp_child_doctor="$(cargo run -q -p wiki-agent --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --viewer-scope private:cli --palace palace.db \
  doctor --tool-backend mcp-child --wiki-cli "$REPO_ROOT/target/debug/wiki-cli")"
echo "$mcp_child_doctor"
echo "$mcp_child_doctor" | grep -q "^tools=22$" || {
  echo "wiki-agent mcp-child doctor did not discover 22 tools" >&2
  exit 1
}

agent_chat="$(cargo run -q -p wiki-agent --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --viewer-scope private:cli --palace palace.db \
  chat "记住：E2E agent memory marker" --web off --fake-llm-response "agent ok")"
echo "$agent_chat"
echo "$agent_chat" | grep -q "agent ok" || {
  echo "wiki-agent fake chat did not return expected response" >&2
  exit 1
}

memory_status="$(cargo run -q -p wiki-agent --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --wiki-dir wiki --viewer-scope private:cli --palace palace.db \
  memory status --json)"
echo "$memory_status"
echo "$memory_status" | grep -q '"pages": 1' || {
  echo "wiki-agent memory page was not written" >&2
  exit 1
}

cargo run -q -p wiki-cli --manifest-path "$REPO_ROOT/Cargo.toml" -- \
  --db wiki.db --viewer-scope private:cli --palace palace.db \
  consume-to-mempalace --last-id 0 >/dev/null
palace_fts_count="$(sqlite3 palace.db "SELECT count(*) FROM drawers_fts WHERE drawers_fts MATCH 'E2E';")"
echo "palace_fts_count=$palace_fts_count"
if [[ "$palace_fts_count" -le 0 ]]; then
  echo "agent memory was not searchable from palace FTS" >&2
  exit 1
fi

echo "E2E PASS: $WORKDIR"
