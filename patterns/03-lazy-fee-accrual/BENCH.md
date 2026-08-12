# Pattern 03 - Lazy Fee Accrual

## Contention problem

The **naive** contract charges a flat fee on every operation by adding it to a
single global accumulator, `FeePool`. Each `operate(who)` writes
`{OpCount(who), FeePool}`. The per-account `OpCount(who)` key is independent, but
`FeePool` is touched by *every* operation of *every* account, so all operations
serialise on it under CAP-0063. The fee bookkeeping alone destroys parallelism
that the underlying operations would otherwise have.

This is the classic "global running total" anti-pattern: a value that is only
ever read in aggregate is nonetheless updated synchronously on every hot-path
write.

## The optimization

The **optimized** contract accrues fees **lazily** on per-account keys:

- Hot path `operate(who)` writes `{OpCount(who), AccruedFee(who)}` -- keys
  private to that account, no global key.
- Cold path `sweep(who)` folds one account's `AccruedFee(who)` into the global
  `FeePool` and resets it, writing `{AccruedFee(who), FeePool}`.

The global total is *materialised on demand* instead of maintained eagerly. This
is the ledger analogue of lazy accrual in accounting: record locally, reconcile
globally when needed. The invariant `sum(swept) + sum(accrued) == naive
FeePool` holds, so no fee is lost.

## Why it improves parallelism (CAP-0063)

- Naive: `M` operations all write `FeePool` -> `M`-clique -> ~`M` stages.
- Optimized: operations by distinct accounts have disjoint footprints and share
  a stage; operations by the *same* account conflict on that account's keys
  (a real dependency). Sweeps still serialise on `FeePool`, but sweeps are
  administrative and infrequent relative to operations, so the hot path stays
  parallel. Contention is moved from the frequent path to the rare path.

## Benchmark methodology (via slipstream-core)

1. `slipstream scan` both variants; capture per-function footprints and any
   `global-accumulator` / `global-write-hotkey` detector findings on `operate`.
2. `slipstream diff naive optimized --json`; confirm `operate`'s
   `writes_delta` drops the global key and `summary.detector_findings_delta`
   is negative.
3. Synthetic batch of `M` operations across `A` accounts (plus a realistic
   sweep cadence, e.g. one sweep per account per epoch). Have slipstream-core
   compute conflict-graph edges and parallel stages for the operation batch
   alone, and separately including sweeps.
4. Report: hot-path stages (expected ~`ceil(M / A)` optimized vs ~`M` naive) and
   the added cost/contention of sweeps. Show the frequency-weighted tradeoff.

## Results

| Metric | Naive | Optimized | Delta |
| --- | --- | --- | --- |
| storage writes (operate) | TBD (not yet measured) | TBD | TBD |
| conflict-graph edges (M ops, A accounts) | TBD | TBD | TBD |
| hot-path parallel stages | TBD | TBD | TBD |
| detector findings | TBD | TBD | TBD |
