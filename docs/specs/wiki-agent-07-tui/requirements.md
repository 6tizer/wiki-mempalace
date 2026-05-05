# wiki-agent 07 TUI Requirements

## Scope

Add a native terminal UI for `wiki-agent` using `ratatui` and `crossterm`.

## Requirements

- `wiki-agent tui` starts the TUI.
- `wiki-agent chat --tui` uses the same TUI runtime.
- Non-TTY execution falls back to CLI chat instead of hanging.
- TUI reuses the existing `ChatRuntime`; it must not bypass ToolRegistry, memory, or write policy paths.
- UI has conversation, activity, input, and status areas.
- Keyboard support: text input, Enter submit, Backspace, Tab panel switch, Up/Down history, Esc/Ctrl-C quit.
- State reducer is testable without a real terminal.
