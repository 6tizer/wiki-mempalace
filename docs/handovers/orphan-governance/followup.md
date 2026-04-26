# Module Handoff: Orphan Governance Follow-up

## Summary

- Upgraded B5 from count-only reporting to `plan -> dry-run/apply`.
- `vault-audit` now scans only `pages/` and `sources/`, writes timestamped
  reports, and exposes path-level evidence.
- `orphan-governance plan` requires a timestamped audit, calls LLM for JSON,
  validates paths/actions, then writes LLM-generated Chinese Markdown.
- `orphan-governance apply` defaults to dry-run and can only insert whitelisted
  frontmatter fields or delete cleanup whitelist paths.

## Public Interfaces

```bash
cargo run -p wiki-cli -- vault-audit \
  --vault /Users/mac-mini/Documents/wiki
```

```bash
cargo run -p wiki-cli -- \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  --llm-config llm-config.toml \
  orphan-governance plan \
  --audit-report /Users/mac-mini/Documents/wiki/reports/vault-audit-<timestamp>.json
```

```bash
cargo run -p wiki-cli -- \
  --wiki-dir /Users/mac-mini/Documents/wiki \
  orphan-governance apply \
  --plan /Users/mac-mini/Documents/wiki/reports/orphan-governance-plan-<timestamp>.json
# add --apply to mutate
```

## Safety Notes

- LLM never writes files. It only returns JSON and Chinese Markdown.
- `apply` revalidates the plan against the timestamped audit before any write.
- `batch-ingest` is never executed by B5 follow-up.
- Root `concepts/` is not created by projection; cleanup belongs to the B5
  whitelist apply path.

## Verification So Far

- `cargo fmt --all -- --check`
- `cargo test -p wiki-cli`
- `cargo test -p wiki-cli --test orphan_governance`
- `cargo test -p wiki-cli --test vault_audit`
- `cargo test -p wiki-cli --test vault_cli_commands`
- `cargo test -p wiki-kernel`
- `cargo test -p wiki-kernel wiki_writer`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo run -p wiki-cli -- llm-smoke --config llm-config.toml --prompt 'Say ok only.'`
- Temp-vault `orphan-governance plan` with real LLM.

## Review Notes

- Review P1 symlink write risk fixed: apply rejects symlink/escaping paths.
- Review P2 Markdown path/command risk fixed: Markdown rejects unknown vault
  paths, code fences, and shell-like commands.
- Review P2 CLI sync root `concepts/` gap fixed with CLI regression test.
- Existing `write_projection` stale managed page cleanup was not changed; it is
  documented existing projection behavior, not B5 cleanup/apply behavior.
