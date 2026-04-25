# Tasks: M12 Strategy Suggestions

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Plain architecture approved
- [x] Plan approved
- [x] Branch created
- [x] Tasks graded as Script / Skill / Agent
- [x] Subagent tasks assigned where needed
- [x] Implementation complete
- [x] Module review complete
- [x] Module handoff written
- [x] Tests added/updated
- [x] Docs updated
- [x] Integration review complete
- [x] PR opened
- [x] Codex/GitHub review addressed
- [x] CI green
- [x] Merged
- [x] Roadmap/PRD updated

## Subtasks

| Task | Grade | Owner | Files | Depends on | Status |
| --- | --- | --- | --- | --- | --- |
| Suggestion model | Agent | Subagent A | `crates/wiki-core/` | Requirements/design approval | Complete |
| Strategy scanner | Agent | Subagent B + main | `crates/wiki-kernel/` | Suggestion model | Complete |
| Report rendering | Agent | Subagent C | `crates/wiki-cli/` | Strategy scanner | Complete |
| CLI output | Agent | Subagent C | `crates/wiki-cli/` | Report rendering | Complete |
| Focused review | Agent | Reviewer D | M12 diff | Implementation complete | Complete; findings addressed |
| Tests/docs | Script / Agent | Main | tests, docs | Review findings addressed | Complete |
| Workflow handoff backfill | Script | Main | `docs/handovers/m12-strategy/summary.md`, this file | PR #16/#17 merged | Complete |

## Review Notes

- Product decisions confirmed:
  - CLI command name: `wiki-cli suggest`.
  - JSON is canonical source of truth.
  - Markdown is a human-readable view rendered from the same report data.
  - Reports are timestamped under `wiki/reports/suggestions/`.
  - M12 first version does not execute suggestions.
  - Internal operator/executor is future scope.
  - Human gate remains required for deletion, discard, force, and disputed
    semantic replacement.
- Alignment checks:
  - M10 `collect_wiki_metrics` is the read-only metrics source for M12.
  - M11 dashboard remains read-only and does not need to consume M12 in first
    version.
  - Schema T2 tags are available but not required for first-version M12 rules.
  - `QueryServed` lacks explicit scope and may contain raw query text; first
    version must skip query-history events whose visibility cannot be proven.

## Deferred Follow-ups

- Internal operator/executor: read `*-m12-suggest.json`, execute allowed
  non-deletion actions, and write separate execution JSON/Markdown reports.
- Dashboard integration: after M12 lands, optionally show or link the latest
  suggestion report from `wiki-cli dashboard`.
- Query history schema: consider adding explicit viewer scope or a true query
  hash to `QueryServed` so future suggestion rules can use history with less
  redaction.

## Handoff

- [docs/handovers/m12-strategy/summary.md](../../handovers/m12-strategy/summary.md)

## Verification

- `cargo test -p wiki-core strategy --quiet`
- `cargo test -p wiki-kernel strategy --quiet`
- `cargo test -p wiki-cli --test suggest --quiet`
- `cargo test -p wiki-cli --test metrics --quiet`
- `cargo test -p wiki-cli --test dashboard --quiet`
- `cargo test -p wiki-core tag --quiet`
- `cargo test -p wiki-kernel tag --quiet`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Draft PR #16 opened.
- PR #16 GitHub `quick` CI passed and merged into main on 2026-04-25.
