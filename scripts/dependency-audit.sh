#!/usr/bin/env bash
set -euo pipefail

out_dir="${DEPENDENCY_AUDIT_OUT_DIR:-artifacts/dependency-audit}"
json_report="$out_dir/dependency-audit.json"
stderr_report="$out_dir/dependency-audit.stderr.txt"

mkdir -p "$out_dir"

if ! command -v cargo-audit >/dev/null 2>&1; then
  echo "cargo-audit is required; install with: cargo install cargo-audit --locked" >&2
  exit 127
fi

set +e
cargo audit --deny warnings --json >"$json_report" 2>"$stderr_report"
status=$?
set -e

if [[ ! -s "$stderr_report" ]]; then
  printf 'cargo audit exited with status %s\n' "$status" >"$stderr_report"
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "## Dependency Audit"
    echo
    echo "- status: \`$status\`"
    echo "- json: \`$json_report\`"
    echo "- stderr: \`$stderr_report\`"
  } >>"$GITHUB_STEP_SUMMARY"
fi

if [[ "$status" -ne 0 ]]; then
  cat "$stderr_report" >&2
fi

exit "$status"
