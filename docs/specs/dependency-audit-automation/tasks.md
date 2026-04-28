# Tasks: Dependency Audit Automation

## Checklist

- [x] PRD created
- [x] Requirements written
- [x] Design written
- [x] Branch: `codex/dependency-audit-automation`
- [x] Add scheduled/manual workflow
- [x] Add `cargo-audit` wrapper script
- [x] Keep quick CI fast with syntax check only
- [x] Handoff
- [x] Local gate
- [x] PR + CI

## Subtasks

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Add dependency audit workflow | Script | Main | `.github/workflows/dependency-audit.yml` | Complete |
| Add wrapper script | Script | Main | `scripts/dependency-audit.sh` | Complete |
| Keep quick CI boundary | Script | Main | `.github/workflows/ci-quick.yml` | Complete |
| Backfill docs/roadmap/LESSONS | Script | Main | `docs/` | Complete |

## Verification

- `bash -n scripts/dependency-audit.sh`
- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Merge

- PR #66 merged on 2026-04-28.
