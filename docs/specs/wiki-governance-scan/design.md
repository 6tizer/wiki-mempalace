# Design: Wiki Governance Scan

## Shape

The scan is split into three layers:

- `wiki-core`: serializable report contract in `governance.rs`.
- `wiki-kernel`: pure scanner over `InMemoryStore` + `DomainSchema`.
- `wiki-cli`: command wiring plus JSON/Markdown report writing.

This keeps PR3 Fixer planning able to read a stable typed JSON report without
depending on CLI stdout text.

## Scanner Sections

- `lifecycle`: page promotion readiness and claim promotion readiness.
- `references`: missing/duplicate/broken source and wikilink evidence.
- `lint`: rendered copy of `collect_basic_lint_findings`.
- `gaps`: rendered copy of `run_gap_scan`.
- `duplicates`: exact source URL, exact body/text, title duplicates, and simple
  token-overlap near-duplicate claim candidates.
- `retire_candidates`: only obvious candidates, such as empty pages, managed
  orphan projection pages, and title-marked merge residue.
- `synthesis_signals`: concept/entity tag counts, pair intersections, distinct
  source domains, existing synthesis topic coverage, and deprecated tag usage.

## Read-only Boundary

The command uses `run_governance_scan(&eng.store, &schema, ...)`.
It does not call:

- `eng.run_basic_lint`, because that mutates audit/outbox state.
- `eng.save_to_repo*`.
- `write_projection`.
- any MemPalace writer.

Report file writes are explicit CLI outputs under `--report-dir`, not wiki
state mutation.
