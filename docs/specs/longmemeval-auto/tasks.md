# Tasks: LongMemEval Auto Benchmark

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
- [ ] PR opened
- [ ] Codex/GitHub review addressed
- [ ] CI green
- [ ] Merged
- [ ] Roadmap/PRD updated

## Subtasks

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Fetch/cache script | Subagent B | `scripts/longmemeval_fetch.sh` | Implemented |
| Runner and scoring | Subagent A | `scripts/longmemeval_run.py`, tests | Implemented |
| Tiny fixture tests | Subagent A | `tests/fixtures/`, tests | Implemented |
| Scheduled workflow | Subagent B | `.github/workflows/longmemeval.yml` | Implemented |
| Review gate | Subagent C | `docs/handovers/longmemeval-auto/review.md` | Pass-with-fixes addressed |
| Docs and status | Main agent | `docs/longmemeval.md`, `docs/prd/batch-3.md`, `docs/roadmap.md` | Updated |
| J14 follow-up note | Main agent | specs / roadmap | Deferred |

## Review Notes

- J13 scope is only the `rust-mempalace` local retrieval baseline.
- Score thresholds are reference-only in J13. Low score writes a report but
  does not fail the workflow.
- Broken runs fail the workflow: script crash, invalid dataset, timeout, or
  missing artifacts.
- J14 Semantic Fusion Benchmark is deferred and should not block J13 merge.

## Verification

- tiny fixture run
- workflow syntax / script compile
- report JSON parses
- markdown report generated from same JSON data
- no `pull_request` trigger in LongMemEval workflow
