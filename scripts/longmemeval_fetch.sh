#!/usr/bin/env bash
set -euo pipefail

DEFAULT_URL="https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json"
CACHE_DIR="${LONGMEMEVAL_CACHE_DIR:-.cache/longmemeval}"
DATASET_URL="${LONGMEMEVAL_DATASET_URL:-$DEFAULT_URL}"
FORCE=0

usage() {
  cat <<'EOF'
Usage: scripts/longmemeval_fetch.sh [--cache-dir DIR] [--url URL] [--force]

Downloads LongMemEval-S cleaned JSON into the local cache.

Env overrides:
  LONGMEMEVAL_CACHE_DIR
  LONGMEMEVAL_DATASET_URL
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --cache-dir)
      CACHE_DIR="${2:?missing value for --cache-dir}"
      shift 2
      ;;
    --url)
      DATASET_URL="${2:?missing value for --url}"
      shift 2
      ;;
    --force)
      FORCE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unexpected argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

mkdir -p "$CACHE_DIR"
DATASET_PATH="$CACHE_DIR/longmemeval_s_cleaned.json"
TMP_PATH="$DATASET_PATH.tmp"

if [[ "$FORCE" -eq 0 && -s "$DATASET_PATH" ]]; then
  echo "cache hit: $DATASET_PATH" >&2
else
  echo "download: $DATASET_URL" >&2
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 --retry-delay 2 --connect-timeout 20 \
      -o "$TMP_PATH" "$DATASET_URL"
  else
    python3 - "$DATASET_URL" "$TMP_PATH" <<'PY'
import sys
import urllib.request

url, out_path = sys.argv[1], sys.argv[2]
with urllib.request.urlopen(url, timeout=60) as response:
    data = response.read()
with open(out_path, "wb") as handle:
    handle.write(data)
PY
  fi
  mv "$TMP_PATH" "$DATASET_PATH"
fi

if [[ ! -s "$DATASET_PATH" ]]; then
  echo "dataset is missing or empty: $DATASET_PATH" >&2
  exit 1
fi

python3 - "$DATASET_PATH" <<'PY'
import json
import os
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as handle:
    data = json.load(handle)

if isinstance(data, (list, dict)) and len(data) > 0:
    print(f"DATASET_PATH={path}")
    print(f"DATASET_BYTES={os.path.getsize(path)}")
    if isinstance(data, list):
        print(f"DATASET_ITEMS={len(data)}")
    else:
        print(f"DATASET_KEYS={len(data)}")
else:
    raise SystemExit(f"dataset JSON must be a non-empty list or object: {path}")
PY
