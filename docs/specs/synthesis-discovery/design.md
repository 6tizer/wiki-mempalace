# Design: Synthesis Discovery

## Shape

- `wiki-core`: report contracts for `SynthesisDiscoveryReport`,
  `SynthesisCandidate`, `SynthesisCandidateKind`, and summary counts.
- `wiki-kernel`: pure discovery over `GovernanceScanReport`.
- `wiki-cli`: `research-synthesis discover`, optional no-engine `--scan` path,
  report rendering, and vault-relative report output.

## Flow

```text
governance scan JSON
-> tag signal map
-> pair intersection map
-> existing synthesis topic coverage
-> single/double/triple/quad candidate pools
-> rank + per-round cap
-> JSON/Markdown report
```

The discovery layer does not call LLM or web search. PR6 consumes these
candidates and performs external research/composition.

## Quad Anchors

Quad candidates require prior synthesis coverage. A three-tag anchor is valid
when either:

- an existing synthesis page has exactly that three-tag set, or
- at least two existing double-tag synthesis pages cover pairs inside that
  triangle.

The fourth tag must have at least 5 concept/entity pages and pairwise
intersection `<= 3` with every anchor tag. This keeps "body" candidates from
collapsing into another near-neighbor cluster.

## Compatibility

- `GovernanceSynthesisSignals` adds `existing_topics` with serde defaults, so
  older scan JSON still deserializes.
- Existing `governance scan` output remains backward-compatible for Fixer.
- No command writes DB state or projection output.
