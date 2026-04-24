# Requirements: LongMemEval Auto Benchmark

## Goal

- Add scheduled, non-blocking LongMemEval retrieval benchmark with artifact reports.

## Functional Requirements

- Add dataset fetch script.
- Add retrieval-only runner.
- Add scheduled/manual GitHub workflow.
- Upload markdown/json artifacts.
- Do not add required PR check.

## Non-Goals

- No PR gate.
- No qa-judge default path in first version.
- No checked-in benchmark dataset.

## Inputs / Outputs

- Input: LongMemEval cleaned dataset downloaded into cache.
- Output: `longmemeval-report.md`, `longmemeval-report.json`, `run-config.json`.

## Acceptance Criteria

- Nightly sample run configured.
- Weekly full retrieval-only run configured.
- Small fixture integration test does not need network.
- Artifacts include R@1, R@5, MRR, sample count, failed cases.

## User / Agent Gates

- User approval needed: thresholds and schedule cadence.
- Agent can automate: scripts, workflow, reports, docs.
