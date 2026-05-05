# wiki-agent 03 CLI Chat Requirements

## Scope

PR3 adds the first usable pure CLI chat surface on top of the native runtime.

## Functional Requirements

- Add `wiki-agent chat [prompt]`.
- Support one-shot prompt and stdin REPL modes.
- Support `--profile <name>` using `llm-config.toml` profiles.
- Support slash commands: `/help`, `/tools`, `/profile`, `/web`, `/memory`, `/sessions`, `/exit`.
- Store sessions and messages in the agent session DB.
- Add `wiki-agent session list` and `wiki-agent session show <id>`.
- Provide fake LLM test path for deterministic tests without provider keys.

## Non-Goals

- No TUI implementation.
- No real tool-calling planner.
- No web RAG.
- No memory/skill extraction into `wiki.db`.

## Acceptance

- Fake LLM can complete a one-shot chat and persist the session.
- `/tools` lists the 22 native tools.
- `/profile <name>` switches the active profile for later turns.
- Session listing shows saved conversations.
