# Design: Benchmark Reproducibility

## CLI

`rust-mempalace bench` adds:

```text
--seed <u64>
```

The main command rejects `--seed` with `--mode fixed`. This avoids misleading reports where a seed is accepted but has no effect.

## Selection

`benchmark_run` accepts `seed: Option<u64>`.

- `mode = "random", seed = Some(N)`: shuffle with `StdRng::seed_from_u64(N)`.
- `mode = "random", seed = None`: keep `rand::rng()` behavior.
- `mode = "fixed"`: preserve current input order.

The helper `choose_benchmark_corpus` owns this selection behavior so it can be tested without depending on search results.

## Storage

`benchmark_runs` gets a nullable additive column:

```sql
seed INTEGER
```

Existing rows remain valid with `seed = NULL`. New seeded random runs write the seed, bounded by SQLite signed integer storage.

## Output

The seed is included in:

- CLI JSON output as `seed`;
- text output when present;
- JSON and Markdown benchmark reports;
- `latest_benchmark` / principles report.

## Compatibility

No existing command invocation changes. Unseeded random benchmarks still behave as before.
