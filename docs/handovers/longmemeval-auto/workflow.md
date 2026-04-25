# LongMemEval Workflow Handover

## Scope

- Owner: Subagent B.
- Files touched:
  - `scripts/longmemeval_fetch.sh`
  - `.github/workflows/longmemeval.yml`
  - `.gitignore`
  - `docs/handovers/longmemeval-auto/workflow.md`

## Implemented

- Added strict-mode dataset fetch/cache script.
- Default dataset URL:
  `https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json`
- Cache path defaults to `.cache/longmemeval/longmemeval_s_cleaned.json`.
- Overrides:
  - `LONGMEMEVAL_CACHE_DIR` or `--cache-dir`
  - `LONGMEMEVAL_DATASET_URL` or `--url`
  - `--force` to refresh cached dataset
- Validation:
  - file exists and is non-empty;
  - Python stdlib `json.load`;
  - top-level JSON must be non-empty list or object.
- `.cache/` is ignored.

## Workflow

- Added `.github/workflows/longmemeval.yml`.
- Triggers:
  - `workflow_dispatch`;
  - nightly cron `0 19 * * *` = Beijing 03:00;
  - weekly cron `0 20 * * 6` = Beijing Sunday 04:00.
- No `pull_request` trigger.
- Manual inputs:
  - `mode`: `nightly`, `weekly`, `manual`, `fixture`;
  - `sample-size`: default `50`; `0` means full dataset.
- `fixture` mode uses `tests/fixtures/longmemeval_tiny.json` directly and skips
  remote dataset fetch.
- Steps:
  - checkout;
  - setup Rust stable;
  - rust-cache;
  - fetch dataset;
  - `python3 -m py_compile scripts/longmemeval_run.py`;
  - `cargo build -p rust-mempalace`;
  - run runner;
  - upload artifacts with `retention-days: 30`.

## Integration Notes

- Workflow assumes runner supports:
  - `--dataset <path>`
  - `--mode <mode>`
  - `--sample-size <n>` for sampled runs
  - `--out-dir <dir>`
- Workflow treats low score as runner/report data, not CI failure.
- Workflow fails on broken runner, invalid dataset, timeout, or missing required
  report files.
- Workflow exports `LONGMEMEVAL_MEMPALACE_BIN=$PWD/target/debug/rust-mempalace`
  so the runner uses the built CLI binary instead of invoking `cargo run` for
  every case.
