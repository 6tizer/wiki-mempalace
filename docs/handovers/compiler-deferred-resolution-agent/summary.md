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
- PR #54 merged.
- Production apply smoke completed on real compiler reports, with DB/Vault/Palace
  post-checks clean.

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

## Model Candidate Note

- Keep `minimax/minimax-m2.7` as the current production compiler default.
- Record `deepseek/deepseek-v4-flash` as a candidate compiler model:
  - OpenRouter route observed: `deepseek/deepseek-v4-flash-20260423`.
  - Small JSON smoke: 1.83s, valid JSON.
  - Temp `Avatar V` compile: success, 117.0s, but core entity `Avatar V`
    stayed machine-deferred.
  - Temp 3-source compile: success=3, failed=0, per-source elapsed 93.6s,
    89.5s, 99.1s.
  - Known quality risks before production switch: near-duplicate concepts such
    as `自愈式浏览器自动化` / `自愈浏览器自动化`, deferred main entities such as
    `Browser Harness`, and Unicode/title cleanup issues such as
    `Magnus M ü ller`.
- Do not switch production compiler model until resolver/fixer covers these
  normalization issues in temp-vault regression.

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
- PR/CI/merge: done in PR #54.
- Production closeout: done. Latest verified state had no new broken wikilinks,
  no new duplicate concept/entity groups, `wiki.db` and `palace.db` integrity
  `ok`, and `consistency-audit` reported `vault_empty_unmanaged=0` /
  `palace_missing_page_drawers=0`.

## Verification

- `cargo fmt --all -- --check` - passed.
- `cargo test -p wiki-cli compiler_resolve_deferred` - passed.
- `cargo test -p wiki-cli run_report_persists_machine_deferred_resolutions` - passed.
- `cargo test --workspace` - passed.
- `cargo clippy --workspace --all-targets -- -D warnings` - passed.
- `git diff --check` - passed.
- Temp X + WeChat apply smoke on `/tmp/wiki-deferred-smoke.*` - passed; DB -> projection -> Mempalace -> lint/audit completed; no `page.broken_wikilink`.
- Production apply report:
  `/Users/mac-mini/Documents/wiki/reports/compiler-deferred-resolution-2026-04-27T15-41-13-182717Z.json`
  - `aliases_applied=1`
  - `pages_created=19`
  - `mempalace unresolved=0`
- Production follow-up dry-run report:
  `/Users/mac-mini/Documents/wiki/reports/compiler-deferred-resolution-2026-04-27T15-42-59-700555Z.json`
  - no additional aliases/creates/changes applied;
  - remaining 4 `keep_deferred` items stayed low-confidence and did not pollute
    the active graph.
