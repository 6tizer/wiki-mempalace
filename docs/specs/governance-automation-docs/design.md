# Design: Governance Automation Docs

## Automation Registry

The existing `AutomationJob` registry remains the single scheduling surface.
PR7 adds five enum variants and job specs:

- `governance-scan`: read-only scan plus JSON/Markdown reports.
- `fixer-plan`: current scan -> typed evidence fixer plan.
- `fixer-apply`: latest fixer plan -> `evidence-auto --apply`; if the latest
  plan is absent, generate a fresh internal-only plan first.
- `synthesis-discover`: current scan -> synthesis candidate report.
- `synthesis-run`: discovery -> compose top candidates -> write verified pages.

`fixer-apply` and `synthesis-run` require the existing writer lease because they
can mutate `wiki.db`, emit outbox, and refresh projection. `governance-scan`,
`fixer-plan`, and `synthesis-discover` are report-only jobs.

## Daily Lane

```text
notion-sync
-> batch-ingest
-> governance-scan
-> fixer-plan
-> fixer-apply
-> maintenance
-> consume-to-mempalace
-> vault-reports
```

`notion-sync` keeps `short_circuit=false` so a temporary Notion outage does not
prevent local maintenance. The following DB-local steps still short-circuit on
failure.

## Report Paths

All default report paths use `--wiki-dir` when available:

```text
<wiki-dir>/reports/governance/
<wiki-dir>/reports/fixer/
<wiki-dir>/reports/synthesis/
```

Without `--wiki-dir`, the same jobs write under `wiki/reports/...`, matching the
existing dashboard/suggest defaults.

## MCP Boundary

MCP remains the live read/write surface for wiki and mempalace tools. Governance
automation is deliberately CLI-only in this PR because `fixer-apply` and
`synthesis-run` are batch jobs with report artifacts, writer lease semantics,
and potentially external search/LLM calls.

## Notion Matrix

`docs/notion-workflow-capability-matrix.md` maps Notion maintenance roles to the
local commands:

- Ingest/Compiler -> `notion-sync`, `batch-ingest`.
- Fixer -> governance scan/plan/apply/restore.
- Synthesis -> discovery/run with internal + web evidence.
- QA -> existing QA boundary, not changed in this batch.
- Projection/Mempalace -> `--sync-wiki`, `consume-to-mempalace`.
