# Contributing to Slipstream Contracts

Thanks for helping grow the Slipstream contention corpus. This repo has two
audiences at once: contract authors reading before/after references, and the
`slipstream-core` analyzer consuming the crates as fixtures. Contributions
should serve both.

## Ground rules

- **No secrets.** Never commit private keys, mnemonics, tokens, or seeds — not
  even in tests or comments. Tests use `soroban_sdk::testutils` generated
  addresses.
- **No fabricated benchmarks.** Do not put invented numbers in any `BENCH.md`.
  Document the *methodology*; leave results as `TBD (not yet measured)` until a
  real `slipstream-core` run produces them.
- **Everything must build and be green** (see Checks below) before a PR.
- **Every crate compiles.** It is fine to leave clearly-scoped future work as
  `// TODO:` with a clean interface and a test asserting the intended shape, but
  never leave code that fails to compile.

## Repository conventions

- Each pattern lives in `patterns/NN-name/` with a `naive/` crate, an
  `optimized/` crate, and a `BENCH.md`.
- Contract crates set `crate-type = ["cdylib", "rlib"]` and depend on
  `soroban-sdk` via the workspace (`soroban-sdk = { workspace = true }`), with
  the `testutils` feature enabled under `[dev-dependencies]`.
- Package names follow `pNN-<pattern>-<variant>` (e.g. `p01-sharded-counter-naive`).
- Every ledger `DataKey` and every public function should carry a doc comment
  stating its **read/write footprint**. That footprint is what the analyzer and
  reviewers reason about.
- Prefer small, deterministic contracts. Each pattern should isolate *one*
  contention anti-pattern.

## Adding a new pattern

1. Create `patterns/NN-name/{naive,optimized}` crates and add both to the
   `members` list in the root `Cargo.toml`.
2. Implement the `naive` anti-pattern and the `optimized` fix. Document the
   footprint of each function.
3. Add `#[cfg(test)]` tests using `soroban_sdk::testutils` and the generated
   contract client that assert **behaviour** (not just compilation).
4. Write `BENCH.md`: the contention problem, the optimization, why it improves
   CAP-0063 parallelism (conflict-graph edges / stage count), and the
   measurement methodology. Results table uses `TBD` placeholders.
5. Update `docs/PLAYBOOK.md`'s taxonomy table and pattern list, and the
   README's "Patterns at a glance" table.

## Checks (run before every PR)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Contracts must build to wasm (see README "Building to wasm" for why v1):
cargo build --workspace --exclude harness --target wasm32v1-none --release
```

CI runs exactly these. `cargo build --workspace` (native) must also succeed.

## Commit / PR notes

- Keep PRs focused (ideally one pattern or one concern).
- Explain the contention behaviour in the PR description, not just the diff.
- Do not run destructive git operations against shared branches.
