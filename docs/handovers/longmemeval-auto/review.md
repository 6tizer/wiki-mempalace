# LongMemEval Auto Review

## Summary

- Focused runner review: production runner does call `rust-mempalace mine/search` as a black box via subprocess, scores R@1/R@5/MRR, skips abstention by default, writes JSON truth plus Markdown from same report object, and records failed cases.
- Focused workflow review: cron UTC maps to Beijing 03:00 nightly and Sunday 04:00 weekly; workflow has `workflow_dispatch`, artifact upload path, 30-day retention, 60-minute timeout, and no `pull_request` trigger.
- Integration review: suggested checks pass. Real tiny fixture can execute end-to-end through `cargo run -p rust-mempalace`, but it returns zero hits for all three non-abstention cases. Current automated test uses a fake CLI, so it does not catch real CLI retrieval regressions.

## Commands Run + Result

| Command | Result |
| --- | --- |
| `python3 -m py_compile scripts/longmemeval_run.py` | Pass |
| `python3 tests/longmemeval_runner_test.py` | Pass: 2 tests OK |
| `bash -n scripts/longmemeval_fetch.sh` | Pass |
| `git diff --check` | Pass |
| Workflow static check for `workflow_dispatch`, cron `0 19 * * *`, cron `0 20 * * 6`, no `pull_request`, retention 30, artifact path, timeout 60 | Pass |
| `LONGMEMEVAL_CACHE_DIR=/tmp/wiki-mempalace-longmemeval-fetch-review LONGMEMEVAL_DATASET_URL=file:///Users/mac-mini/wiki-migration/wiki-mempalace/tests/fixtures/longmemeval_tiny.json scripts/longmemeval_fetch.sh` | Pass; wrote dataset only under `/tmp/wiki-mempalace-longmemeval-fetch-review/` |
| `python3 scripts/longmemeval_run.py --dataset tests/fixtures/longmemeval_tiny.json --out-dir /tmp/wiki-mempalace-longmemeval-review/out --mode fixture --sample-size 3 --top-k 5 --repo-root .` | Pass process exit 0; artifacts written; report has `sample_count=3`, `R@1=0.0`, `R@5=0.0`, `MRR=0.0`, `failed_count=3`, returned ids empty for q1/q2/q3 |

## Findings

| Priority | File/Line | Issue | Recommendation |
| --- | --- | --- | --- |
| P2 | `tests/longmemeval_runner_test.py:22` | The only automated fixture test replaces `rust-mempalace` with a fake CLI. Production runner uses the real CLI, but test coverage does not prove the committed tiny fixture works against `rust-mempalace mine/search`. Manual real-CLI run completed but returned no results for q1/q2/q3, so the fake test can mask retrieval-contract breakage. | Keep the fake CLI test for metric math, but add a real CLI smoke/integration test or adjust the tiny fixture so at least one case returns a real hit through `rust-mempalace` black-box search. |
| P2 | `scripts/longmemeval_run.py:148` | CLI subprocess calls have no per-command timeout, while `timeout_count` is always reported as `0` at `scripts/longmemeval_run.py:299`. If one case hangs, the workflow-level 60-minute timeout kills the job with no partial report and a misleading timeout field in successful runs. | Add runner-level timeout config around `subprocess.run`, count timed-out cases, include them in failed cases, and keep workflow timeout as outer guard. |
| P3 | `.github/workflows/longmemeval.yml:15` | Manual input exposes `fixture` mode, but workflow always fetches the remote LongMemEval dataset and passes that path to the runner. Selecting `fixture` does not run `tests/fixtures/longmemeval_tiny.json`, so the input name is misleading. | Either remove `fixture` from workflow choices or route fixture mode to the checked-in tiny fixture and skip remote fetch. |
| P3 | `docs/specs/longmemeval-auto/tasks.md:9` | Spec task status is stale: subagent assignment, implementation, tests, docs, and integration review remain unchecked, and subtasks still say `Planned` at lines 25-29 despite implemented files and handovers. | Before PR, update tasks checklist and subtask statuses to match actual module state and this review verdict. |

## Fix Follow-up

Main agent addressed the `pass-with-fixes` findings:

- Added a real `rust-mempalace` CLI fixture smoke test and adjusted the tiny
  fixture queries so at least one real black-box retrieval hit is required.
- Added per-command runner timeout support and `runtime.timeout_count`.
- Routed workflow `fixture` mode to `tests/fixtures/longmemeval_tiny.json`
  instead of fetching the remote dataset.
- Updated `docs/specs/longmemeval-auto/tasks.md` implementation/review status.

Follow-up verification:

| Command | Result |
| --- | --- |
| `python3 -m py_compile scripts/longmemeval_run.py` | Pass |
| `python3 tests/longmemeval_runner_test.py` | Pass: 3 tests OK |
| `bash -n scripts/longmemeval_fetch.sh` | Pass |
| Workflow static check for fixture path and no `pull_request` | Pass |

## Verdict

pass
