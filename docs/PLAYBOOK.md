# Slipstream Contention Playbook

This playbook is the conceptual companion to the `slipstream-contracts` corpus.
It explains the contention taxonomy the patterns demonstrate, walks each of the
six patterns, and describes how the corpus feeds the `slipstream-core`
analyzer.

## Background: parallel execution on Stellar (CAP-0063)

Stellar's phased/parallel execution lets validators run transactions in a ledger
concurrently. The scheduler partitions a batch into **stages**; transactions in
the same stage must have **non-conflicting footprints**. A transaction's
*footprint* is the set of ledger keys it reads and writes. Two transactions
**conflict** when one writes a key the other reads or writes.

The practical consequences that this corpus is built around:

- **A key written by every transaction is a global bottleneck.** It forces those
  transactions into separate stages -> serial execution.
- **False sharing serialises independent work.** Co-locating unrelated state in
  one ledger entry makes unrelated transactions conflict.
- **Footprint width matters.** A wide write-footprint collides with more of the
  batch, so it is harder to schedule.
- **The goal of optimization is to make the footprint conflict graph match the
  *true* data dependencies** of the workload, and no more.

Slipstream measures how close a contract gets to that goal.

## Contention taxonomy

The corpus is organised around recurring contention anti-patterns. Each pattern
directory contains a `naive/` crate exhibiting the anti-pattern and an
`optimized/` crate applying the fix, plus a `BENCH.md` with the analysis and
measurement methodology.

| # | Anti-pattern | Symptom | Fix family |
| --- | --- | --- | --- |
| 01 | **Global write hot key** | one key written by every call | **Sharding** — spread writes over N independent keys |
| 02 | **False sharing in a container** | all state in one map/vec under one key | **Key-per-entity** — give each entity its own ledger key |
| 03 | **Eager global accumulator** | a running total updated on every op | **Lazy accrual** — record locally, reconcile on demand |
| 04 | **Global monotonic counter** | one persistent counter bumped by everyone | **Per-user + temporary storage** — scope uniqueness, evict cheaply |
| 05 | **Wide write-footprint** | many keys written together in a loop | **Batching** — one struct under one key |
| 06 | **Shared append tail** | one tail/count key read+written by every append | **Per-writer segments** — one tail pointer per writer |

These families are not mutually exclusive; real contracts often combine them
(e.g. per-user keys that also batch that user's fields into one struct).

### Pattern → contention mapping

The table below summarises each pattern's contention class, the ledger-key
shape each side demonstrates, and the expected CAP-0063 stage effect. Key
shapes use the `slipstream-core` vocabulary (dotted static keys; `(dynamic)`
marks a key the static analyzer cannot fully resolve at compile time). Rows
link to the measured-deltas methodology in each pattern's `BENCH.md`.

| Pattern | Contention class | Naive key shape | Optimized key shape | Expected stage / parallelism effect |
| --- | --- | --- | --- | --- |
| 01-sharded-counter ([BENCH.md](patterns/01-sharded-counter/BENCH.md)) | Global write hot key | `Counter` — one key read+written by every `increment` (`read-modify-write`) | `("shard", i)` — N disjoint shard keys | increments to distinct shards touch disjoint keys and run in the same stage; only `total()` reads all N |
| 02-per-user-balance ([BENCH.md](patterns/02-per-user-balance/BENCH.md)) | False sharing in a container | `Balances` — whole map under one key, rewritten on every deposit/transfer | `Balance(addr)` — one key per account | deposits to distinct accounts are independent (parallel); transfers conflict only on shared endpoints |
| 03-lazy-fee-accrual ([BENCH.md](patterns/03-lazy-fee-accrual/BENCH.md)) | Eager global accumulator | `(dynamic)` + `FeePool` — shared RMW on every `operate` | per-writer `(dynamic)` record; `FeePool` only on rare `sweep` | distinct writers' ops run in parallel; the shared key is touched only by reconciliation |
| 04-temporary-nonce ([BENCH.md](patterns/04-temporary-nonce/BENCH.md)) | Global monotonic counter | `Nonce` — one counter RMW on every `next` | per-user `(dynamic)` key + temporary storage | distinct users hold disjoint keys; uniqueness is scoped per user and evicts cheaply |
| 05-batched-admin-writes ([BENCH.md](patterns/05-batched-admin-writes/BENCH.md)) | Wide write-footprint | `(dynamic)` — many keys written in a loop per `set_config` (`write-in-loop`) | `Config` — one key holding the whole struct | the write-footprint collapses from N keys to one, shrinking conflict-graph surface per op |
| 06-event-log ([BENCH.md](patterns/06-event-log/BENCH.md)) | Shared append tail | `Tail` — one pointer read+written by every append (`read-modify-write`) | `SegmentTail(segment)` — one per-writer tail pointer | appends to distinct segments touch disjoint keys and share a stage; only reads aggregate across segments |

## The patterns

### 01 - Sharded counter
- **Naive:** single `Counter` persistent key; every `increment()` conflicts.
- **Optimized:** `increment(shard)` writes one of N `("shard", i)` keys;
  `total()` sums them. Writers to different shards never conflict.
- **Lesson:** turn one hot key into N cold keys; pay for it with a rare
  read-side aggregation.

### 02 - Per-user balance
- **Naive:** one `Balances` map under one key; every deposit/transfer rewrites
  the whole map.
- **Optimized:** `Balance(addr)` per account. Deposits to different accounts are
  independent; transfers conflict only on shared endpoints.
- **Lesson:** eliminate false sharing so contention reflects real dependencies.

### 03 - Lazy fee accrual
- **Naive:** every `operate()` bumps a global `FeePool`.
- **Optimized:** accrue fees on per-account `AccruedFee(who)` keys on the hot
  path; fold into `FeePool` only on the rare `sweep(who)`.
- **Lesson:** move contention from the frequent path to the infrequent path.

### 04 - Temporary nonce
- **Naive:** one persistent monotonic `Nonce` key for everyone.
- **Optimized:** per-account nonce in *temporary* storage; uniqueness is scoped
  per signer, and the key is cheaply evicted.
- **Lesson:** scope uniqueness to where it is actually required, and pick the
  cheapest storage class that satisfies the invariant.

### 05 - Batched admin writes
- **Naive:** `set_config` writes each field to its own key in a loop (wide
  footprint).
- **Optimized:** the whole config is one struct under one `Config` key (narrow,
  fixed footprint).
- **Lesson:** collapse state that always changes together into one entry to
  minimise conflict-graph surface area.

### 06 - Segmented event log
- **Naive:** every `append` reads and writes the single shared `Tail` pointer,
  serialising all appends behind one hot key.
- **Optimized:** `append(segment, msg)` touches only
  `SegmentTail(segment)` and that segment's entry; concurrent appends to
  distinct segments never conflict.
- **Lesson:** split a shared append point into per-writer segments; pay for it
  with a wider read-side aggregation (`total_len`).

## How the corpus feeds slipstream-core

`slipstream-core` is a static/analytic engine (sibling Rust repo) that reads
contract source, extracts each function's storage footprint, runs contention
**detectors**, and can diff two contracts. This corpus is its **test corpus**
and its **demonstration set**:

1. **Ground truth for detectors.** Each `naive/` crate is a known-positive for a
   specific detector (global write hot key, false sharing, eager accumulator,
   global monotonic counter, wide write-footprint). Each `optimized/` crate is
   the corresponding known-negative. Detector regressions show up as a naive
   crate that stops tripping, or an optimized crate that starts.
2. **Diff fixtures.** `slipstream diff naive optimized --json` over each pair
   should show reduced storage writes / detector findings. The `harness/` crate
   consumes exactly this JSON (see below).
3. **Footprint / conflict-graph fixtures.** Because the intended footprints are
   documented per function in each crate's doc comments and `BENCH.md`, the
   corpus doubles as expected-output fixtures for the footprint extractor and
   the conflict-graph / stage-count model.

### The harness

`harness/` is a small Rust crate that drives `slipstream-core` over a pattern
pair and reports the **contention delta**.

- It defines a `CoreRunner` trait with two implementations:
  - `SubprocessRunner` shells out to a real `slipstream` binary (path from
    `SLIPSTREAM_BIN`, default `slipstream` on `PATH`).
  - `MockRunner` returns canned JSON, used by the harness's own unit tests so
    the delta logic is testable without `slipstream-core` present.
- `slipstream-core` is **not vendored** here. Running the harness against real
  core requires the `slipstream` binary to be installed and on `PATH`.
- The JSON shapes the harness deserializes (`DiffReport`, `AnalysisReport`) are
  defined in `harness/src/model.rs` and mirror the documented `slipstream-core`
  output contract.

### Expected detector findings

`slipstream scan --json` over each `naive/` crate is pinned by the integration
test `harness/tests/detector_conformance.rs` (run with `SLIPSTREAM_BIN` set;
it skips cleanly otherwise). The expected findings below are the single source
of truth for that test and reflect the detectors `slipstream-core` ships today:

| Naive contract | Expected detectors |
| --- | --- |
| 01-sharded-counter | `read-modify-write` on `Counter` |
| 02-per-user-balance | *(none)* — the whole `Balances` map lives under one key, but that false-sharing is not statically detectable as a single key today |
| 03-lazy-fee-accrual | `read-modify-write` on `FeePool` and `(dynamic)` |
| 04-temporary-nonce | `read-modify-write` on `Nonce` |
| 05-batched-admin-writes | `write-in-loop` on `(dynamic)` |
| 06-event-log | `read-modify-write` on `Tail` |

When a detector evolves, update both the test table and this one.

## Measurement discipline

- **No fabricated numbers.** `BENCH.md` result tables are populated only from
  real `slipstream-core` runs. The `slipstream-harness bench` command runs each
  complete pair and writes the measured deltas (with full provenance) into the
  `## Measured deltas` block of each `BENCH.md`, so the numbers are
  reproducible and comparable across patterns.
- The methodology in each `BENCH.md` is the contract for *how* a number is
  produced (footprint sizes, conflict-graph edges, parallel-stage counts).
