# Requirements: LongMemEval Auto Benchmark

## Goal

- Add a scheduled, non-blocking LongMemEval benchmark for the
  `rust-mempalace` local retrieval path.
- Treat J13 as a baseline exam: it measures whether mempalace can find the
  expected long-term memory and rank it near the top. It does not answer the
  question, learn from failures, or auto-fix retrieval.

## Functional Requirements

- Add a dataset fetch/cache script that downloads LongMemEval data into
  `.cache/longmemeval/`.
- Do not check the benchmark dataset into the repository.
- Add a retrieval-only runner for `rust-mempalace`.
- Configure GitHub Actions with:
  - manual `workflow_dispatch`;
  - nightly sample run at 03:00 Asia/Shanghai;
  - weekly full run at Sunday 04:00 Asia/Shanghai.
- Nightly run uses 50 questions.
- Weekly run uses the full available dataset.
- Upload benchmark artifacts with 30-day retention.
- Do not add LongMemEval as a required PR check.
- Record reference thresholds, but do not fail the workflow only because
  retrieval scores are low.
- Fail the workflow only when the run is unusable, for example script crash,
  invalid/missing dataset, report write failure, or timeout.

## Non-Goals

- No PR gate.
- No qa-judge default path in first version.
- No checked-in benchmark dataset.
- No `wiki-cli query --vectors --palace-db` lane in J13.
- No external embedding or LLM rerank in the J13 benchmark lane.
- No automatic retrieval tuning, ingest repair, or score-driven code changes.

## Inputs / Outputs

- Input: LongMemEval cleaned dataset downloaded into cache.
- Input: a temporary or configured `rust-mempalace` data directory populated
  from the benchmark case.
- Output: `longmemeval-report.md`.
- Output: `longmemeval-report.json`.
- Output: `run-config.json`.
- Output: failed-case details with bounded fields such as case id, query,
  expected id, returned top ids, and short snippets.

## Acceptance Criteria

- Nightly sample run configured.
- Weekly full retrieval-only run configured.
- Small fixture integration test does not need network.
- Artifacts include R@1, R@5, MRR, sample count, failed cases.
- Artifacts include runtime health fields: `total_runtime_sec`,
  `avg_query_ms`, `throughput_per_sec`, `timeout_count`.
- Workflow artifact retention is 30 days.
- Pull request CI remains unchanged: LongMemEval is not triggered by
  `pull_request` and is not listed as a required check.

## User / Agent Gates

- User approved:
  - nightly 50-question sample run;
  - weekly full run;
  - 03:00 Asia/Shanghai nightly cadence;
  - Sunday 04:00 Asia/Shanghai weekly cadence;
  - 30-day artifact retention;
  - reference thresholds only, not score-based workflow failure;
  - J13 scope limited to `rust-mempalace` local retrieval.
- Agent can automate: scripts, workflow, reports, docs.

## Deferred Follow-up: J14 Semantic Fusion Benchmark

J14 is a separate future module, not part of J13 acceptance.

J14 should benchmark the semantic fusion path such as
`wiki-cli query --vectors --palace-db`. It can start only after:

- J13 is merged.
- At least 7 valid nightly reports exist.
- At least 1 valid weekly full report exists.
- The weekly full run does not time out.
- The J13 artifact format is stable.
- The real full-run duration is known.

J14 should start earlier if failed cases show many synonym or wording-mismatch
problems that local retrieval cannot handle well. It should not start if the
main failures are dataset loading, ingest, scope, or runner bugs.
