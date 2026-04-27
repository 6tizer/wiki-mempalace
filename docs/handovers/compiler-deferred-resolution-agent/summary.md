# Module Handoff: Compiler Deferred Resolution Agent

## Summary

- Seeded minimal PRD/spec trio for machine-only deferred resolution.
- Scope locked to production-wiki-compiler JSON `deferred_resolutions`.
- Decisions allowed: alias existing, safe create new canonical, ignore noise,
  keep machine-deferred.
- No human/manual lane.
- Apply order locked: `wiki.db -> Vault projection -> Mempalace consume/replay
  -> lint/audit report`.
- Implemented `compiler-resolve-deferred`.
- Compiler run JSON now emits deferred candidate `page_id`.

## Planned CLI

```bash
compiler-resolve-deferred --report <json> [--apply] [--allow-create] [--report-dir <path>]
```

Use global:

```text
--db --wiki-dir --sync-wiki --viewer-scope --palace
```

## Regression Boundary

- Temp vault/DB/palace only.
- X + WeChat only.
- Real `/Users/mac-mini/Documents/wiki` blocked unless user explicitly
  approves.

## Files Seeded

| File | Purpose |
| --- | --- |
| `docs/prd/compiler-deferred-resolution-agent.md` | Scope source |
| `docs/specs/compiler-deferred-resolution-agent/requirements.md` | Behavior contract |
| `docs/specs/compiler-deferred-resolution-agent/design.md` | CLI/data/apply design |
| `docs/specs/compiler-deferred-resolution-agent/tasks.md` | Plan/tasks |
| `docs/handovers/compiler-deferred-resolution-agent/summary.md` | Handoff |
| `crates/wiki-cli/src/compiler_deferred.rs` | Deferred resolver core |
| `crates/wiki-cli/src/wiki_compiler.rs` | Deferred candidate `page_id` in run report |
| `crates/wiki-cli/src/main.rs` | CLI command and apply pipeline |

## Status

- PRD/spec/branch/plan/subagent assignment: done.
- Implementation/tests/temp smoke: done.
- Focused/integration review: done.

## Verification

- `cargo fmt --all -- --check` - passed.
- `cargo test -p wiki-cli compiler_resolve_deferred` - passed.
- `cargo test -p wiki-cli run_report_persists_machine_deferred_resolutions` - passed.
- `cargo test --workspace` - passed.
- `cargo clippy --workspace --all-targets -- -D warnings` - passed.
- `git diff --check` - passed.
- Temp X + WeChat apply smoke on `/tmp/wiki-deferred-smoke.*` - passed; DB -> projection -> Mempalace -> lint/audit completed; no `page.broken_wikilink`.
