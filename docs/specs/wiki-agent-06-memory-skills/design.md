# wiki-agent 06 Memory + Skills Design

## Flow

```text
agent_messages
-> explicit marker extractor
-> verifier
-> duplicate check against wiki.db pages
-> writer lease
-> WikiPage write
-> projection
-> outbox flush
```

## Candidate Kinds

- `fact`: explicit `记住:` / `remember:` lines.
- `skill`: explicit `技能:` / `skill:` lines.

Fact candidates become concept pages titled `记忆：...`.
Skill candidates become skill pages titled `技能：...`.

## Safety

The verifier rejects:

- password/API-key/token-like content.
- prompt-injection phrases.
- bidi/invisible control characters.
- empty or oversized candidates.

## Storage

Raw transcript remains in `.wiki/wiki-agent.db`.
Durable memory lands in `wiki.db` as pages, then normal projection/outbox paths
make it available to Vault and Mempalace.
