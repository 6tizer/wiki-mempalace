# wiki-agent 07 TUI Design

## Runtime

The TUI wraps the same `chat::ChatRuntime` used by CLI chat.

```text
key events
-> TuiApp reducer
-> ChatRuntime::run_prompt
-> evidence / answer
-> TuiApp transcript + activity
```

## Layout

- left panel: conversation transcript.
- right panel: tool/evidence/activity feed.
- bottom panel: input composer.
- status row: status, active panel, token count, tool count.

## Fallback

If stdin or stdout is not a TTY, `wiki-agent tui` and `wiki-agent chat --tui`
fall back to normal CLI chat. This keeps tests and pipes deterministic.

## Safety

TUI never writes directly to `wiki.db`, Vault, or `palace.db`.
All writes still pass through `ChatRuntime`, memory curator, workers, or
ToolRegistry-backed commands.
