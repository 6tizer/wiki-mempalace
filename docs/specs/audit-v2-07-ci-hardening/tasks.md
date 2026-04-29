# Tasks: Audit v2 PR 07 CI Hardening

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-07-ci-hardening/` | Complete |
| Required clippy gate | Script | Main | `.github/workflows/ci-quick.yml` | Complete |
| Cargo-deny config + CI step | Agent | Main | `deny.toml`, `.github/workflows/ci-quick.yml` | Complete |
| Workflow/docs consistency | Script | Main | `docs/dev-workflow.md`, `.github/pull_request_template.md`, roadmap/spec index | Complete |
| Focused review + gates | Main | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] `quick` job includes workspace clippy
- [x] `quick` job includes cargo-deny
- [x] `cargo audit` lane remains scheduled/manual
- [x] `deny.toml` covers advisories, yanked, licenses, duplicates, sources
- [x] Focused review complete
- [x] `cargo deny --all-features check advisories bans licenses sources`
- [x] `bash -n scripts/dependency-audit.sh`
- [x] `cargo fmt --all -- --check`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
