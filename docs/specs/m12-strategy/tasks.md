# Tasks: M12 Strategy Suggestions

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Plan approved
- [x] Branch created
- [x] Subagent tasks assigned
- [x] Implementation complete
- [x] Module review complete
- [x] Tests added/updated
- [x] Docs updated
- [x] Integration review complete
- [x] PR opened
- [ ] Codex/GitHub review addressed
- [ ] CI green
- [ ] Merged
- [x] Roadmap/PRD updated

## Subtasks

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Suggestion model | Subagent A | `crates/wiki-core/` | Complete |
| Strategy scanner | Subagent B + main | `crates/wiki-kernel/` | Complete |
| Report rendering | Subagent C | `crates/wiki-cli/` | Complete |
| CLI output | Subagent C | `crates/wiki-cli/` | Complete |
| Focused review | Reviewer D | M12 diff | Complete; findings addressed |
| Tests/docs | Main | tests, docs | Complete |

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
