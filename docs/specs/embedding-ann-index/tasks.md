# Tasks: ANN-Backed wiki_embedding Search

## Checklist

- [ ] Requirements approved
- [ ] Design approved (extension choice + CI/release story)
- [ ] Plan approved
- [ ] Branch: `codex/embedding-ann-index`
- [ ] Spike: load `sqlite-vec` (or chosen) in dev build
- [ ] Implement upsert + search dual path
- [ ] Tests: feature on/off, recall or exact match policy
- [ ] Docs: operator notes (extension path, fallbacks)
- [ ] Handoff: `docs/handovers/embedding-ann-index/summary.md`
- [ ] PR + CI (optional feature job) green
- [ ] PRD / roadmap updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Technology spike + build matrix | Agent | TBD | `crates/wiki-storage/`, `Cargo.toml` | Spec approval | Not started |
| Index DDL + migration in `open` | Agent | TBD | `wiki-storage` | Spike | Not started |
| `upsert` / `delete` + index consistency | Agent | TBD | `wiki-storage` | DDL | Not started |
| `search_embeddings_cosine` ANN path | Agent | TBD | `wiki-storage` | Index | Not started |
| Fall back + warn / metrics | Script | TBD | `wiki-storage` | — | Not started |
| Long fixture / bench (optional) | Script | TBD | `benches/` or `tests/` | ANN path | Not started |

## Review Gates

- No regression with default feature set (CI).
- If ANN is approximate, document tolerance and re-rank strategy.

## Stop Conditions

- If extension cannot be distributed for macOS/linux CI within budget, freeze
  at “chunked scan + `LIMIT`” and revisit — update PRD with user sign-off.

## Verification

- `cargo test -p wiki-storage`
- `cargo test --workspace` (feature on/off as matrix if added)
