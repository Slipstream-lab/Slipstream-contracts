# Pattern 01 - Sharded Counter

## Contention problem

The **naive** counter keeps a single value under one persistent ledger key,
`Counter`. Every `increment()` call reads and writes that one key, so the
*write-footprint* of every counting transaction is exactly `{Counter}`.

Under Stellar's phased/parallel execution (CAP-0063), the scheduler groups
transactions into stages such that transactions within a stage have disjoint
read/write footprints. Two transactions that both write `Counter` conflict, so
they can never share a stage: a stream of `increment()` calls is forced to
execute **serially**, one per stage, regardless of how many cores are
available. The single key is a global write-contention hot key.

## The optimization

The **optimized** counter spreads the count across `N` independent persistent
keys, `("shard", 0) .. ("shard", N-1)`. `increment(shard)` writes exactly one
shard key, so its write-footprint is `{("shard", shard)}`. Callers spread
traffic across shards (e.g. `hash(account) % N`).

- Increments to *different* shards have disjoint footprints and can be placed in
  the **same** parallel stage.
- `total()` reads all shards and sums them; it is the read-side aggregation that
  pays for contention-free writes and is expected to be rare.

## Why it improves parallelism (CAP-0063)

Model the batch as a conflict graph: one node per transaction, an edge when two
transactions' footprints intersect on a written key. Stage count is bounded
below by the graph's structure (a clique must be fully serialised).

- Naive: all `M` increments write `Counter`, forming an `M`-clique. Minimum
  stages ~= `M` (fully serial on the counter).
- Optimized: increments partition across `N` shards. Only same-shard increments
  conflict, so the graph is `N` disjoint cliques. Minimum stages ~=
  `max shard occupancy` ~= `ceil(M / N)` for balanced traffic.

So the theoretical serialization drops from `M` to about `M / N`. `total()`
conflicts with every writer (it reads all shard keys), which is why it belongs
off the hot path.

## Benchmark methodology (via slipstream-core)

No numbers are asserted here. To measure, one would:

1. `slipstream scan patterns/01-sharded-counter/naive --json` and the same for
   `optimized`; record `storage_writes` per function and any
   `global-write-hotkey` detector findings.
2. `slipstream diff patterns/01-sharded-counter/naive patterns/01-sharded-counter/optimized --json`
   and read `summary.storage_writes_delta` and
   `summary.detector_findings_delta`.
3. Build a synthetic batch of `M` increments (uniformly distributed over shards
   for the optimized case) and have slipstream-core compute the footprint
   conflict graph, reporting: number of conflict edges, and the number of
   parallel stages (graph colouring / greedy stage assignment).
4. Report edges and stage count for both variants; expect fewer edges and fewer
   stages for the optimized variant. Sweep `N` (shard count) to show the
   stage-count vs. shard-count tradeoff.

## Results

Run `harness/` against a real `slipstream` binary to populate this table.

| Metric | Naive | Optimized | Delta |
| --- | --- | --- | --- |
| storage writes (increment) | TBD (not yet measured) | TBD | TBD |
| conflict-graph edges (M increments) | TBD | TBD | TBD |
| parallel stages (M increments) | TBD | TBD | TBD |
| detector findings | TBD | TBD | TBD |
