# Handover: Audit Disposition PR3 MCP API Reference

## Scope

I-8 from the 2026-04-30 audit disposition: refresh MCP API reference and add
docs sync tests.

## Changed Files

- `docs/mcp-api-reference.md`
- `crates/wiki-cli/src/mcp.rs`
- `docs/prd/audit-disposition-2026-04-30.md`
- `docs/specs/audit-disposition-03-mcp-api-reference/*`
- `docs/roadmap.md`
- `docs/specs/README.md`
- `docs/LESSONS.md`

## Behavior

- API reference now lists all current wiki and mempalace MCP tools.
- Reference documents required/optional args, writes, returns, typed errors,
  scope rules, limit clamp, projection, and bank capability.
- Docs sync tests fail if tool names disappear from the reference or mempalace
  table arg columns expose client `bank_id`.

## Verification

Passed local gates on 2026-04-30:

- `cargo test -p wiki-cli mcp_api_reference -- --nocapture`
- `git diff --check`
- `cargo fmt --all -- --check`
- `cargo deny --all-features check advisories bans licenses sources`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Focused Review

- Scope: on target.
- Review depth: standard.
- Hard stops: 0.
- `bank_id` appears only as rejection/capability wording, not as a mempalace client arg.

## Production Safety

No production `/Users/mac-mini/Documents/wiki` write operation is part of this PR.
