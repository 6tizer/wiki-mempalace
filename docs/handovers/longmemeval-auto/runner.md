# LongMemEval Runner Handover

## Scope

- Owner: Subagent A.
- Files changed:
  - `scripts/longmemeval_run.py`
  - `tests/fixtures/longmemeval_tiny.json`
  - `tests/longmemeval_runner_test.py`
  - `docs/handovers/longmemeval-auto/runner.md`

## Implementation

- Added stdlib-only Python runner for retrieval-only J13 baseline.
- Loads JSON array or JSONL LongMemEval cleaned records.
- Validates required fields:
  - `question_id`
  - `question_type`
  - `question`
  - `haystack_session_ids`
  - `haystack_dates`
  - `haystack_sessions`
  - `answer_session_ids`
- Skips abstention cases by default when `question_id` ends with `_abs` or
  `question_type` marks abstention.
- Uses deterministic sampling by sorted `question_id`.
- Creates one temp `rust-mempalace` palace per case.
- Writes one text file per haystack session and calls CLI `mine`.
- Calls CLI `search --output json`, maps `source_path` back to session id, then
  scores R@1, R@5, and MRR.
- Applies a per-command timeout around each CLI call. Timed-out cases are
  counted in `runtime.timeout_count` and included in failed cases.

## Artifacts

Runner writes:

- `longmemeval-report.json`
- `longmemeval-report.md`
- `run-config.json`
- `failed-cases.jsonl`

JSON report is source of truth. Markdown is generated from same report object.
Low score does not fail. Broken run exits non-zero.

## Verification

Commands:

```bash
python3 -m py_compile scripts/longmemeval_run.py
python3 tests/longmemeval_runner_test.py
```

The Python test uses a fake CLI via `LONGMEMEVAL_MEMPALACE_BIN` to stay offline
and deterministic while preserving the `mine` / `search` interface shape.
It also includes a real `rust-mempalace` CLI smoke test against the tiny fixture.
