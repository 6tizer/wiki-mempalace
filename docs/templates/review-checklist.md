# Review Checklist

## Focus

- Spec compliance
- Correctness
- Regression risk
- Scope creep
- Missing tests
- Docs drift
- Handoff completeness

## Findings

| Priority | File | Finding | Decision |
| --- | --- | --- | --- |
|  |  |  |  |

## Required Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Feature-specific tests:
- [ ] Spec checklist updated
- [ ] Module handoff present if subagent touched code

## Decision

- [ ] Approve
- [ ] Request changes
- [ ] Defer with tracking note
