# wiki-agent 03 CLI Chat Design

## Design

`chat.rs` owns the CLI runtime:

- Builds a `SessionStore`.
- Creates or resumes a session.
- Handles one-shot or REPL input.
- Routes slash commands locally.
- Uses `WikiAiChatModel` for real LLM calls and `FakeChatModel` for tests.

`session_store.rs` owns `.wiki/wiki-agent.db`:

- `agent_sessions`
- `agent_messages`

The chat runtime does not write durable memory into `wiki.db` in PR3. That is
reserved for PR6 after verifier and injection checks exist.

## Tool Visibility

`/tools` uses native discovery from `ToolRegistry`. It is not a real tool-call
planner yet; PR4/PR5 will add evidence routing and worker orchestration.
