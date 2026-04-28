# PRD: J14 Semantic Fusion Benchmark

## Goal

Extend the LongMemEval lane with a side-by-side semantic fusion comparison:
query-only baseline vs. semantic-fusion retrieval.

## Scope

- Add an opt-in runner flag: `--compare-semantic-fusion`.
- Run both retrieval variants against the same mined case data.
- Keep `semantic_fusion` as the primary report metrics for comparison runs.
- Add `metrics_by_variant` to JSON and a Variant Metrics section to Markdown.
- Wire scheduled/manual LongMemEval workflow to run the comparison lane.

## Non-Goals

- Required PR CI.
- Score-based workflow failure.
- External paid embeddings or LLM judging.
- Changing LongMemEval dataset fetch/cache behavior.

## Success Criteria

- Broken runs still fail: script crash, invalid dataset, timeout, missing report.
- Low benchmark score does not fail workflow.
- Fixture test proves `query_baseline` and `semantic_fusion` metrics are reported
  separately.
- Existing non-comparison report contract remains compatible.

## Status

- **Complete PR #75** — runner, workflow, fixture test, docs, and GitHub quick check completed.
