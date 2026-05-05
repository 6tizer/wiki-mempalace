# Handoff: wiki-agent 03 CLI Chat

## Scope

Branch: `codex/wiki-agent-cli-chat`.

Implemented PR3 of the wiki-agent batch:

- Added `wiki-agent chat [prompt]`.
- Added REPL mode with `/help`, `/tools`, `/profile`, `/web`, `/memory`, `/sessions`, and `/exit`.
- Added `ChatModel` abstraction with real `wiki-ai` profile-backed model and fake test model.
- Added SQLite session store at the agent session DB path.
- Added `wiki-agent session list` and `wiki-agent session show <id>`.
- Added CLI tests for one-shot chat, `/tools`, and `/profile`.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p wiki-agent`
- `cargo clippy -p wiki-agent --all-targets -- -D warnings`
- manual smoke: one-shot fake LLM returns answer
- manual smoke: REPL `/tools` lists 22 native tools

## Notes

- PR3 intentionally does not implement TUI, web RAG, sub-agent routing, or durable memory extraction.
- `/memory` only reports the current boundary; PR6 will add verified durable memory and skill writes.
- `/tools` lists the native registry; real automatic tool planning starts in PR4/PR5.
