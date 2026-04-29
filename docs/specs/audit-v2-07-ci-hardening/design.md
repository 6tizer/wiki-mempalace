# Design: Audit v2 PR 07 CI Hardening

## Required PR Gate

The repository currently has one required PR check named `quick`. To make new
required checks effective without relying on branch-protection reconfiguration,
`ci-quick.yml` keeps the `quick` job and adds mandatory steps inside it:

- workspace `cargo fmt`;
- workspace `cargo test`;
- workspace `cargo clippy -D warnings`;
- ANN feature smoke;
- `cargo-deny` policy check;
- shell syntax and automation smoke.

If any new required check fails, the existing `quick` check fails.

## Cargo Deny Policy

`deny.toml` is the source of truth for fast supply-chain policy:

- advisories and yanked crates are denied;
- only explicitly allowed SPDX licenses are accepted;
- unknown registries and git sources are denied;
- duplicate versions are denied except for documented current transitive
  duplicates that cannot be removed without dependency upgrades.

The duplicate skip list is explicit and version-pinned so future duplicates are
not silently accepted.

## Heavy Audit Boundary

`.github/workflows/dependency-audit.yml` remains scheduled/manual. It still owns
`cargo audit` artifacts and does not run on pull requests. PR7 adds a faster
required `cargo-deny` gate, not a second required `cargo audit` lane.

## Compatibility

No runtime behavior changes. CI will take longer because `quick` now includes
clippy and cargo-deny.
