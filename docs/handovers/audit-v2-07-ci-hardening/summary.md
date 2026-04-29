# Audit v2 PR 07 CI Hardening Handoff

## Branch

`codex/audit-v2-07-ci-hardening`

## Status

Implemented locally. Verification passed.

## Completed

- Added clippy to the existing required `quick` GitHub Actions job.
- Added `cargo-deny` to the same `quick` job so advisory/license/source/duplicate
  policy failure fails the required PR check.
- Added root `deny.toml` with explicit license/source/advisory policy and a
  version-pinned duplicate exception list.
- Upgraded `rustls-webpki` from `0.103.12` to `0.103.13` because cargo-deny
  surfaced `RUSTSEC-2026-0104`.
- Left scheduled/manual `cargo audit` workflow unchanged.
- Backfilled spec trio, roadmap, spec index, workflow docs, PR template, and
  lessons.

## Modified Files

- `.github/workflows/ci-quick.yml`
- `.github/pull_request_template.md`
- `Cargo.lock`
- `deny.toml`
- `docs/dev-workflow.md`
- `docs/prd/dependency-audit-automation.md`
- `docs/specs/audit-v2-07-ci-hardening/`
- `docs/handovers/audit-v2-07-ci-hardening/summary.md`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/LESSONS.md`

## Interfaces

- GitHub check `quick` now includes clippy and cargo-deny.
- Local required gate adds:
  `cargo deny --all-features check advisories bans licenses sources`.

## Known Limits

- Branch-protection settings are not changed by code. This PR relies on the
  existing required `quick` check.
- Duplicate dependency remediation is deferred; current exceptions are explicit
  in `deny.toml`.

## Verification

Passed:

- `cargo deny --all-features check advisories bans licenses sources`
- `bash -n scripts/dependency-audit.sh`
- `ruby -e "require 'yaml'; YAML.load_file('.github/workflows/ci-quick.yml')"`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

Focused review found no P0/P1/P2 findings.
