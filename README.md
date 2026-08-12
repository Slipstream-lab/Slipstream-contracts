# Slipstream Contracts

Reference contract corpus for **Slipstream** — an analytical engine (sibling
repo [`slipstream-core`](https://github.com/Slipstream-lab/Slipstream-core),
Rust) that measures how well Soroban contract transaction-footprints
parallelize under Stellar's phased execution
([CAP-0063](https://stellar.org/protocol/cap-0063)).

Each **pattern** demonstrates a contention anti-pattern in a `naive`
implementation and the corresponding fix in an `optimized` implementation. The
repo doubles as the analyzer's test/demonstration corpus.

> Any performance figures anywhere in this repository are **illustrative**
> unless produced by a real `slipstream-core` run. Results tables ship with
> `TBD (not yet measured)` placeholders on purpose; see each pattern's
> `BENCH.md` for the measurement methodology.

## What this repo is

Soroban contracts on Stellar execute in parallel stages; transactions can only
share a stage if their ledger read/write **footprints** don't conflict. A key
written by every transaction, false sharing of unrelated state in one entry, or
an over-wide write-footprint all serialise work that could otherwise run
concurrently.

This corpus isolates those anti-patterns one at a time so that:

- contract authors have copy-pasteable before/after references, and
- `slipstream-core` has ground-truth positives (`naive`) and negatives
  (`optimized`) for its detectors, diffs, and conflict-graph model.

See [`docs/PLAYBOOK.md`](docs/PLAYBOOK.md) for the full contention taxonomy.

## Layout

```
slipstream-contracts/
├── Cargo.toml                      # workspace (resolver v2)
├── rust-toolchain.toml             # stable + rustfmt/clippy + wasm target
├── patterns/
│   ├── 01-sharded-counter/         # global write hot key  -> sharding
│   │   ├── naive/                  #   single COUNTER key
│   │   ├── optimized/              #   N sharded keys + total()
│   │   └── BENCH.md
│   ├── 02-per-user-balance/        # false sharing          -> key-per-entity
│   ├── 03-lazy-fee-accrual/        # eager accumulator       -> lazy accrual
│   ├── 04-temporary-nonce/         # global monotonic counter-> per-user + temp
│   └── 05-batched-admin-writes/    # wide write-footprint    -> batched struct
├── harness/                        # runs a pattern pair through slipstream-core
└── docs/PLAYBOOK.md
```

Each contract is its own crate (`crate-type = ["cdylib", "rlib"]`) depending on
`soroban-sdk = "27"`.

## Patterns at a glance

| # | Pattern | Anti-pattern | Fix |
| --- | --- | --- | --- |
| 01 | sharded-counter | global write hot key | shard writes across N keys |
| 02 | per-user-balance | false sharing in one map | one ledger key per account |
| 03 | lazy-fee-accrual | eager global accumulator | per-account lazy accrual + sweep |
| 04 | temporary-nonce | global persistent monotonic counter | per-user nonce in temporary storage |
| 05 | batched-admin-writes | wide write-footprint | one config struct under one key |

## Building and testing

Tests use `soroban_sdk::testutils` (the SDK's native test environment) — **no
`stellar`/`soroban` CLI is required**.

A `Makefile` mirrors CI (run `make help` to list targets):

```bash
make check   # fmt + clippy + native tests
make wasm    # build the contract cdylibs for wasm32v1-none
make all     # everything CI runs
```

The underlying commands:

```bash
# Run every contract's behavioural tests and the harness unit tests.
cargo test --workspace

# Lint and format (must be clean; CI enforces both).
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Building to wasm

soroban-sdk 27 does **not** support `wasm32-unknown-unknown` on modern Rust.
Its `build.rs` errors on `wasm32-unknown-unknown` for Rust >= 1.82 (that target
enables the `reference-types` and `multi-value` wasm features, which the Soroban
host rejects) and directs you to **`wasm32v1-none`**, the supported wasm target
from Rust 1.84 onward. Build the contract crates with that target:

```bash
rustup target add wasm32v1-none
# The harness is a native binary (it shells out to slipstream-core), so exclude
# it from the wasm build; only contract crates target wasm.
cargo build --workspace --exclude harness --target wasm32v1-none --release
```

Artifacts land in `target/wasm32v1-none/release/*.wasm`.

> If you are pinned to a toolchain where `wasm32-unknown-unknown` is still the
> required target name for your tooling, you would need Rust <= 1.81 — but
> soroban-sdk 27's dependency tree requires edition 2024, so 1.81 and earlier
> cannot build it. On any toolchain that can build soroban-sdk 27,
> `wasm32v1-none` is the correct wasm target. The CI workflow reflects this.

## The harness

[`harness/`](harness/) drives `slipstream-core` over a `naive`/`optimized` pair
and reports the contention delta (change in storage writes / reads / detector
findings).

```bash
# Requires a real `slipstream` binary on PATH (or set SLIPSTREAM_BIN).
# slipstream-core is NOT vendored in this repo.
cargo run -p harness --bin slipstream-harness -- patterns/01-sharded-counter
```

It abstracts core behind a `CoreRunner` trait: `SubprocessRunner` (real binary)
and `MockRunner` (canned JSON for tests). The delta computation is unit-tested
against the mock, so `cargo test -p harness` passes with no external binary.

## Related

- [`slipstream-core`](https://github.com/Slipstream-lab/Slipstream-core) — the
  analytical engine that consumes this corpus.
- [CAP-0063](https://stellar.org/protocol/cap-0063) — Stellar parallel execution.
- [Soroban SDK](https://docs.rs/soroban-sdk) — the contract SDK.

## License

MIT © 2026 Slipstream Lab. See [LICENSE](LICENSE).
