# Design: Evidence Fixer Plan

## Shape

The planner is split into three layers:

- `wiki-core`: typed plan/action contracts plus semantic patch proposal parser.
- `wiki-kernel`: pure planner from `GovernanceScanReport` to
  `EvidenceFixerPlan`.
- `wiki-cli`: no-engine command wiring, optional web verification, and
  JSON/Markdown report writing.

This keeps PR4 apply/restore able to consume the same typed plan without parsing
CLI text.

## Planner Rules

- Lifecycle eligible signals become `promote_status`.
- `page.incomplete` lint becomes `add_missing_section`.
- `page.empty_title` lint becomes `set_title_from_h1`.
- Duplicate claim source references become `dedupe_source_reference`.
- Raw URL reference upgrades stay blocked until PR4 can recheck current page
  state and exact replacement source.
- Exact duplicate groups become ready `merge_duplicate`.
- Near duplicate groups use a deterministic verification key. They become ready
  only when a web run crosses the provider/domain threshold.
- Retire candidates become ready `retire_page` with tombstone requirement.
- Deprecated tag replacement stays blocked until a replacement tag is supplied.
- Semantic patch proposals are ready only when source chain and verifier pass
  are present.

## Web Boundary

The CLI performs optional near-duplicate verification before calling the kernel
planner:

```text
scan duplicate group
-> query from labels/key
-> dual provider web_search runtime
-> cross_verified key set
-> pure planner
```

No web query is sent unless the user passes `--allow-web-search`. Private scopes
also require `--allow-private-web-search`.

## Read-only Boundary

`fixer-plan` is handled in `commands::dispatch::maybe_run_without_engine`, before
runtime opens a repository. It reads only input JSON files and optional semantic
proposal JSON. Report files are explicit CLI outputs under `--report-dir`, not
wiki state mutation.
