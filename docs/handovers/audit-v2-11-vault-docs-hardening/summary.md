# Handover: Audit v2 PR 11 Vault Docs Hardening

## Scope

PR 11 closes the remaining P3 audit follow-up items: Vault projection safety,
docs consistency, and a scheduled/manual hardening lane.

## Changed Files

- `crates/wiki-kernel/src/wiki_writer.rs`
- `.github/workflows/hardening.yml`
- `.github/workflows/ci-quick.yml`
- `scripts/hardening-smoke.sh`
- `docs/specs/audit-v2-11-vault-docs-hardening/*`
- `docs/architecture.md`
- `docs/README.md`
- `docs/vault-standards.md`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/LESSONS.md`

## Behavior

- New projection pages include `managed_by: wiki-mempalace` and
  `managed_kind: wiki-page-projection`.
- Projection cleanup only touches files with that marker and a valid UUID id.
- Projection write targets are guarded: unmarked existing files fail instead of
  being overwritten; other managed projection files are quarantined first.
- Projection rejects symlinked `pages/` / entry-type parent directories before writing.
- Root `index.md` / `log.md` projection rejects symlink targets and uses guarded atomic replace.
- Stale/obsolete managed pages are moved to `.wiki/trash/projection/`.
- Hand-written UUID pages without the marker are preserved.
- Titles that slug to empty use `page-{id8}.md`.
- YAML frontmatter strings escape control characters safely.
- Hardening workflow runs perf smoke, MCP malformed/boundary, DB corruption, CJK,
  and bank/scope lanes on schedule/manual dispatch only.

## Verification

Completed local gates:

- `cargo test -p wiki-kernel wiki_writer -- --nocapture`
- `bash -n scripts/hardening-smoke.sh`
- `bash scripts/hardening-smoke.sh db-corruption`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo deny --all-features check advisories bans licenses sources`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Production Safety

No production `/Users/mac-mini/Documents/wiki` write operation is part of this PR.
