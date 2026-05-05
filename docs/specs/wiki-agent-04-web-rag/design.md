# wiki-agent 04 Web RAG Design

## Flow

```text
user prompt
-> native wiki_query internal evidence
-> planner decides web off/auto/always
-> private-scope web guard
-> web search run or fake web evidence
-> evidence pack rendered to CLI and injected into LLM prompt
-> assistant answer
```

## Modules

- `planner.rs`: web mode and private-scope policy.
- `evidence.rs`: internal + web evidence rendering and prompt context.
- `web_tool.rs`: live `wiki_ai::web_search::run_search` plus fake artifact loader.
- `answer.rs`: builds the evidence-backed user prompt.

## Boundaries

Internal evidence currently uses native `wiki_query`, which records the same
`QueryServed` event as MCP query. Web evidence must be cross-verified before it
enters the answer prompt. Private scope blocks web even when a fake evidence file
is supplied, matching the real privacy boundary.
