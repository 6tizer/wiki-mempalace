# PRD: Compiler Deferred Resolution Agent

**Status**: implemented on branch `codex/compiler-deferred-resolution-agent`
**Related**: `Production Wiki Compiler`, `Compiler Canonicalization v2`

## Goal

Consume production-wiki-compiler JSON `deferred_resolutions` and turn safe
machine decisions into durable wiki state.

## Scope

- Read compiler run reports containing `deferred_resolutions`.
- For each deferred item, decide one of:
  - alias to existing canonical page,
  - create new canonical page only when safe,
  - ignore noise,
  - keep machine-deferred when confidence is low.
- No human/manual lane.
- Emit JSON/Markdown report for audit.

## Apply Order

```text
wiki.db
 -> Vault projection
 -> Mempalace consume/replay
 -> lint/audit report
```

`wiki.db` remains source of truth. Do not patch generated Vault Markdown or
`palace.db` directly.

## CLI Shape

```bash
cargo run -p wiki-cli -- \
  --db <path> \
  --wiki-dir <path> --sync-wiki \
  --viewer-scope <scope> \
  --palace <path> \
  compiler-resolve-deferred --report <json> [--apply] [--allow-create] [--report-dir <path>]
```

## Regression Guard

- Regression uses temp vault/DB/palace only.
- Fixtures cover X + WeChat only.
- Never touch real `/Users/mac-mini/Documents/wiki` unless user explicitly
  approves.

## Non-Goals

- No broad production compile.
- No manual review queue.
- No direct Vault repair.
- No direct Mempalace DB patch.
- No archived source retirement.

## Success Criteria

- Safe aliases persist to canonical wiki data.
- Safe creates require `--allow-create`.
- Low-confidence cases remain machine-deferred.
- Noise is ignored with report evidence.
- Apply run follows DB -> Vault -> Mempalace -> audit.
