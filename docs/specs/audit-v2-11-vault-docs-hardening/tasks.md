# Tasks: Audit v2 PR 11 Vault Docs Hardening

| Task | Grade | Owner | Files | Status |
| --- | --- | --- | --- | --- |
| Spec trio | Script | Main | `docs/specs/audit-v2-11-vault-docs-hardening/` | Complete |
| Projection ownership marker + quarantine | Agent | Main | `crates/wiki-kernel/src/wiki_writer.rs` | Complete |
| Projection write collision guard | Agent | Main | `crates/wiki-kernel/src/wiki_writer.rs` | Complete |
| Empty slug fallback + YAML escape | Agent | Main | `crates/wiki-kernel/src/wiki_writer.rs` | Complete |
| Focused projection tests | Agent | Main | `crates/wiki-kernel/src/wiki_writer.rs` tests | Complete |
| Hardening workflow + script | Agent | Main | `.github/workflows/hardening.yml`, `scripts/hardening-smoke.sh` | Complete |
| Docs consistency updates | Script | Main | `docs/architecture.md`, `docs/README.md`, `docs/vault-standards.md`, `docs/roadmap.md`, `docs/specs/README.md`, `docs/LESSONS.md` | Complete |
| Docs/slow-lane review + gates | Main/Subagent | workspace | Complete |

## Checklist

- [x] Behavior matches roadmap scope
- [x] empty slug falls back to `page-{id8}`
- [x] YAML frontmatter escapes newline/tab/control chars
- [x] projection writes explicit managed marker
- [x] projection rejects unmanaged target collision without overwrite
- [x] projection rejects symlinked entry-type directories before write
- [x] root `index.md` / `log.md` projection rejects symlink targets
- [x] stale managed pages are quarantined, not deleted
- [x] hand-written UUID page without marker is preserved
- [x] hardening lane covers perf smoke, MCP malformed/boundary, DB corruption, CJK, bank/scope
- [x] `bash -n scripts/hardening-smoke.sh`
- [x] `bash scripts/hardening-smoke.sh db-corruption`
- [x] `cargo test -p wiki-kernel wiki_writer -- --nocapture`
- [x] docs consistency + slow lane review complete
- [x] `cargo fmt --all -- --check`
- [x] `git diff --check`
- [x] `cargo deny --all-features check advisories bans licenses sources`
- [x] `cargo test --workspace`
- [x] `cargo clippy --workspace --all-targets -- -D warnings`
