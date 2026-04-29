#!/usr/bin/env bash
# Scheduled/manual hardening smoke lanes. These are intentionally outside the
# required PR quick gate.

set -euo pipefail

lane="${1:-all}"

run_lane() {
  case "$1" in
    perf)
      cargo test -p wiki-storage reliability_large_snapshot_roundtrip_smoke -- --nocapture
      ;;
    mcp-boundary)
      cargo test -p wiki-cli mcp::tests:: -- --nocapture
      ;;
    db-corruption)
      tmpdir="$(mktemp -d)"
      bad_db="$tmpdir/wiki.db"
      out="$tmpdir/automation-health.out"
      printf 'not a sqlite database\n' > "$bad_db"
      if cargo run -q -p wiki-cli -- --db "$bad_db" automation health >"$out" 2>&1; then
        echo "corrupt DB unexpectedly passed automation health" >&2
        exit 1
      fi
      grep -Eiq 'database|sqlite|malformed|error|failed' "$out"
      ;;
    cjk-retrieval)
      cargo test -p rust-mempalace e2e_cli_cjk_unicode_search -- --nocapture
      cargo test -p rust-mempalace cjk_ -- --nocapture
      ;;
    bank-scope)
      cargo test -p wiki-cli mempalace_ -- --nocapture
      cargo test -p rust-mempalace bank -- --nocapture
      cargo test -p wiki-mempalace-bridge scope -- --nocapture
      ;;
    all)
      for item in perf mcp-boundary db-corruption cjk-retrieval bank-scope; do
        run_lane "$item"
      done
      ;;
    *)
      echo "unknown hardening lane: $1" >&2
      exit 2
      ;;
  esac
}

run_lane "$lane"
