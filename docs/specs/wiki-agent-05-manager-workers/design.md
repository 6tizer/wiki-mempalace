# wiki-agent 05 Manager-Worker Design

## Public Command

```bash
wiki-agent agent run "lint this wiki" --json
wiki-agent agent run "find synthesis candidates" --task synthesis --json
wiki-agent agent run "fix safe issues" --task fixer --apply --json
```

`--task` overrides manager routing. Without `--task`, the manager chooses a role
from the prompt.

## Runtime

```text
goal
-> Manager::plan
-> AgentTask
-> WorkerRuntime::execute
-> WorkerReport
```

## Native Role Mapping

- `lint_agent`: `collect_basic_lint_findings`; this avoids the recorded `wiki_lint`
  path because MCP lint writes an audit/report.
- `search_agent`: `query_ranked_with_ports`; this avoids the recorded `wiki_query`
  path because MCP query writes `QueryServed`.
- `governance_agent`: `run_governance_scan`.
- `fixer_agent`: `run_governance_scan` -> `build_evidence_fixer_plan`; optional
  `--apply` runs `apply_evidence_fixer_plan` behind writer lease.
- `synthesis_agent`: `run_governance_scan` -> `discover_synthesis_candidates`.
- `memory_curator`: blocked until PR6.

## Fallback

`mcp-child` can call only existing MCP tools through `wiki-cli mcp --once`.
Therefore `lint_agent` and `search_agent` may use fallback tool calls, while
governance/fixer/synthesis/memory return blocked reports unless native backend
is available.

## Writer Lease

Only apply tasks request a writer lease. Read workers can run concurrently in the
same process because they open independent SQLite readers and do not mutate the
engine snapshot.
