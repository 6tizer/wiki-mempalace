# rust-mempalace

MemPalace core ideas implemented as a local-first Rust CLI:

- Store raw verbatim text (never summarize on write)
- Organize memory as `wing / hall / room`
- Build links across wings via explicit `tunnel`
- Retrieve with local SQLite FTS5 search
- Generate compact wake-up context (L0 + L1)

## Why this version can be better

This implementation keeps MemPalace's strongest idea (`store everything + make it navigable`) while making the stack easier to run and ship:

- single static CLI binary
- zero Python runtime requirements
- transparent heuristic classification (easy to audit)
- SQLite FTS5 for portable local search
- Hybrid retrieval fallback (FTS miss => LIKE fallback)
- Simple rerank on top of recall candidates
- Lexical + trigram semantic blend scoring
- Taxonomy + tunnel traversal for navigable memory graph
- Split large transcript exports before mining
- Configurable classifier rules (`classifier_rules.json`)
- `mine --mode convos` for conversation-style ingestion

## Quick start

```bash
cargo run -- init --identity "You are my coding copilot. Preserve architecture decisions."
cargo run -- mine ~/Projects/some-repo
cargo run -- mine ~/chat-exports --mode convos
cargo run -- search "why did we choose postgres"
cargo run -- taxonomy
cargo run -- wake-up
cargo run -- status
cargo run -- bench --samples 30 --top-k 5
```

## Commands

```bash
mempalace-rs --palace ~/.mempalace-rs init [--identity "..."]
mempalace-rs --palace ~/.mempalace-rs mine <path> [--mode projects|convos] [--wing ...] [--hall ...] [--room ...]
mempalace-rs --palace ~/.mempalace-rs search "<query>" [--wing ...] [--hall ...] [--room ...] [--limit 8]
mempalace-rs --palace ~/.mempalace-rs wake-up [--wing ...]
mempalace-rs --palace ~/.mempalace-rs status
mempalace-rs --palace ~/.mempalace-rs link --from-wing ... --from-room ... --to-wing ... --to-room ...
mempalace-rs --palace ~/.mempalace-rs taxonomy
mempalace-rs --palace ~/.mempalace-rs traverse --wing ... --room ...
mempalace-rs --palace ~/.mempalace-rs split ./huge_transcript.txt [--marker "### Session"] [--min-lines 20] [--dry-run]
mempalace-rs --palace ~/.mempalace-rs bench [--samples 50] [--top-k 5] [--report ./bench.md|./bench.json]
```

## Data model

- `drawers`: verbatim content and metadata (`wing/hall/room/source_path/hash`)
- `drawers_fts`: FTS5 index for retrieval
- `tunnels`: explicit cross-wing links
- implicit tunnels: same `room` across different `wing` during `traverse`

## Notes

- Classification is deterministic and explainable, driven by path/text keywords.
- You can edit `~/.mempalace-rs/classifier_rules.json` to customize wing/hall routing.
- Duplicate content is skipped using a stable SHA256 hash.
