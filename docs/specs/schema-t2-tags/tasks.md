# Tasks: Schema T2 Tag Governance

## Checklist

- [x] Requirements approved
- [x] Design approved
- [x] Plan approved
- [x] Branch created
- [x] Subagent tasks assigned
- [x] Implementation complete
- [x] Module review complete
- [x] Tests added/updated
- [x] Docs updated
- [x] Integration review complete
- [x] PR opened
- [x] Codex/GitHub review addressed
- [x] CI green
- [x] Merged
- [x] Roadmap/PRD updated

## Subtasks

| Task | Owner | Files | Status |
| --- | --- | --- | --- |
| Add model tags | Subagent | `crates/wiki-core/` | Done |
| Add normalization/validation | Subagent | `crates/wiki-core/`, `crates/wiki-kernel/` | Done |
| Wire ingest paths | Subagent | `crates/wiki-kernel/`, `crates/wiki-cli/` | Done |
| Tests/docs | Subagent | tests, docs | Done |

## Review Notes

- Local implementation complete on `codex/schema-t2-tags`.
- Core adds `tags` to `Claim`, `RawArtifact`, and `LlmClaimDraft` with serde defaults for old JSON.
- Tag policy now normalizes tags, rejects `deprecated_tags`, and errors when `max_new_tags_per_ingest` is exceeded.
- Kernel adds `ingest_raw_with_tags` and `file_claim_with_tags`; old APIs still write empty tags.
- CLI/MCP/batch paths preserve source tags and claim tags, with preflight validation to avoid partial writes.
- Focused core/kernel/CLI tests pass. Integration review and workspace fmt/test/clippy pass. PR #13 is merged into main.

## Verification

- Focused tests passed for core/kernel/wiki-cli tag, MCP, and batch paths.
- Passed integration gate:
  - `cargo fmt --all -- --check`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
