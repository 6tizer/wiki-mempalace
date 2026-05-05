# Handoff: wiki-agent 07 TUI

## Scope

Branch: `codex/wiki-agent-tui`.

Implemented:

- `wiki-agent tui`.
- `wiki-agent chat --tui`.
- ratatui + crossterm terminal loop.
- state reducer for input/history/panel switching.
- conversation/activity/input/status layout.
- non-TTY fallback to CLI chat.
- shared `ChatRuntime` turn execution for CLI and TUI.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p wiki-agent`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
- `git diff --check`

After terminal-session cleanup fix:

- `cargo fmt --all -- --check`
- `cargo test -p wiki-agent`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo deny --all-features check advisories bans licenses sources`
- `git diff --check`

## Review

- No P0/P1 findings in local self-review.
- Dependency policy note: `ratatui` is configured with `std` only and `ratatui-crossterm` is pinned to the `crossterm_0_28` feature to avoid the unmaintained `paste` advisory and crossterm 0.29 runtime shift.
- `deny.toml` adds narrow duplicate-package skips for `hashbrown@0.16.1` and Windows-only target packages introduced by the TUI stack; `cargo deny` passes after the update.
