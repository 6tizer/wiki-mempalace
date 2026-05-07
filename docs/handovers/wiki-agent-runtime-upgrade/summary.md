# Handoff: wiki-agent Runtime Upgrade

Branch: `codex/wiki-agent-snapshot-harness-tests`.

## Scope

This closeout PR finishes the runtime-upgrade batch after PR #125-#134:

- Adds regression coverage for harness event order, retry bounds, private web
  block status, TUI message rendering, and wide/narrow layout behavior.
- Adds the runtime-upgrade spec trio.
- Updates PRD, roadmap, spec index, and lessons for the completed batch.

## Validation

Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p wiki-agent
cargo clippy -p wiki-agent --all-targets -- -D warnings
git diff --check
```

After merge:

```bash
vera update . --onnx-jina-cpu --exclude 'target/**' --exclude '.git/**' --exclude '.vera/**' --json
npx gitnexus analyze --force --skip-agents-md
```

## Notes

- `HarnessRuntime.run` was not changed in PR11 because GitNexus impact marks it
  HIGH; coverage was added around the existing behavior instead.
- Native tools remain default; MCP child remains compatibility fallback.
- `.wiki/wiki-agent.db` remains raw transcript/session storage; durable memory
  still writes only through the verified memory path.
