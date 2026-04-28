# Requirements: J14 Semantic Fusion Benchmark

## Functional Requirements

- **R1 comparison flag**: Runner supports `--compare-semantic-fusion`.
- **R2 shared setup**: Both variants use the same mined case data and bank.
- **R3 variants**: Reports include `query_baseline` and `semantic_fusion`.
- **R4 primary metrics**: Top-level metrics stay compatible and use
  `semantic_fusion` for comparison runs.
- **R5 non-blocking score**: Low scores never fail the workflow; broken runs do.
- **R6 artifacts**: JSON, Markdown, config, and failed-case artifacts remain
  generated.

## Acceptance Criteria

- [x] Python fixture test covers side-by-side variant metrics.
- [x] `run-config.json` records comparison mode.
- [x] Workflow can pass `--compare-semantic-fusion`.
- [x] PR + CI green.

## Checklist

- [x] Existing non-comparison runner behavior preserved
- [x] No new network path in PR tests
- [x] No required PR check
