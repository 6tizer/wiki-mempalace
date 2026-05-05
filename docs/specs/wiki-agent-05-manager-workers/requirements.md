# wiki-agent 05 Manager-Worker Requirements

## Scope

PR5 adds a local Manager-Worker runtime for `wiki-agent`. It must use native Rust
direct calls first and only use MCP child-process as a compatibility fallback for
tools already exposed by the 22-tool MCP surface.

## Functional Requirements

- Add a manager that turns a user goal into a typed worker task.
- Support worker roles:
  - `lint_agent`
  - `governance_agent`
  - `fixer_agent`
  - `synthesis_agent`
  - `search_agent`
  - `memory_curator`
- Workers must report input, role, status, tool calls, artifacts, blockers, and
  machine-readable output.
- Native workers must call shared Rust APIs:
  - pure kernel/storage read APIs for lint and search workers.
  - `wiki-kernel` governance scan, evidence fixer plan/apply, and synthesis discovery.
  - Existing PR4 search evidence path for search tasks where web is enabled.
- Read workers may run without a writer lease.
- Write workers must acquire the existing SQLite writer lease before apply.
- MCP child fallback may call only existing MCP tools.
- Batch workers that do not exist in the 22 MCP tools must return a typed blocked report under `mcp-child`.

## Non-Goals

- No autonomous long-running task queue.
- No durable memory write loop; `memory_curator` is routed but blocked until PR6.
- No TUI worker tree; TUI remains PR7.

## Acceptance

- Manager routes lint/governance/fixer/synthesis/search/memory prompts to the expected role.
- Native lint/search workers stay read-only without requiring a writer lease.
- Native governance/fixer/synthesis workers call kernel APIs.
- `mcp-child` governance/fixer/synthesis workers are blocked with clear reason.
- Fixer apply is blocked when the writer lease is already held.
- JSON reports are stable and parseable.
