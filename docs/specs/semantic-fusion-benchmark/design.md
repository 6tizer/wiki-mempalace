# Design: J14 Semantic Fusion Benchmark

## Summary

J14 extends the existing J13 LongMemEval runner. It does not create a new
workflow family or introduce external embeddings. Instead, the runner can run
two local retrieval variants per case:

- `query_baseline`: lexical-weighted retrieval with vector/RRF weights disabled.
- `semantic_fusion`: current `rust-mempalace` retrieval defaults with sparse
  semantic vector + reciprocal-rank fusion weights enabled.

## Flow

1. Write benchmark sessions.
2. Mine sessions once into a case-local palace.
3. Write `config.json` for `query_baseline`, run search, score.
4. Write `config.json` for `semantic_fusion`, run search, score.
5. Store primary top-level case result from `semantic_fusion`.
6. Store both variant rows under `variant_results`.
7. Aggregate `metrics_by_variant`.

## Artifact Contract

Existing fields remain:

- `metrics`
- `runtime`
- `failed_cases`
- `cases`

New comparison fields:

- `comparison_enabled`
- `primary_variant`
- `metrics_by_variant`
- per-case `variant_results`

## Failure Semantics

Same as J13:

- Broken run fails workflow.
- Low score writes artifacts but does not fail.

## Test Strategy

- `python3 -m py_compile scripts/longmemeval_run.py`
- `python3 tests/longmemeval_runner_test.py`
- `bash -n scripts/longmemeval_fetch.sh`
