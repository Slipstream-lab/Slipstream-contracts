# Pattern 04 - Temporary Nonce

## Contention problem

The **naive** contract hands out replay-protection nonces from a single
**persistent** monotonic counter, `Nonce`. Every `next()` call from every
account reads and writes that one key, so all nonce requests conflict on it under
CAP-0063 and serialise -- a global bottleneck introduced solely to guarantee
uniqueness.

There is a second cost: the key is **persistent**, so it occupies ledger state
indefinitely and accrues rent, even though a nonce only needs to be unique among
an account's own in-flight transactions.

## The optimization

The **optimized** contract gives each account its own nonce stored in
**temporary** storage under `Nonce(addr)`:

- `next(who)` writes only `{Nonce(who)}` -- private to the account.
- Nonces are monotonic *per account*, which is exactly what replay protection
  requires (uniqueness is per-signer, not global).
- Temporary storage means the entry is cheaply evicted once it stops being
  touched, so replay-protection state does not accumulate persistent rent.

## Why it improves parallelism (CAP-0063)

- Naive: all `M` `next()` calls write the one `Nonce` key -> `M`-clique ->
  ~`M` stages (fully serial).
- Optimized: `next(who)` for distinct accounts have disjoint footprints and
  share a stage. Only repeated calls by the same account (which are inherently
  ordered anyway) conflict. Stage count drops to roughly the max number of
  concurrent nonce requests by any single account.

Choosing *temporary* over *persistent* storage additionally keeps the state
footprint bounded and reduces long-term ledger pressure, which is a
parallelism-adjacent win (smaller, shorter-lived keys are easier for the host to
manage).

## Benchmark methodology (via slipstream-core)

1. `slipstream scan` both variants; capture the storage class (persistent vs
   temporary) and footprint of `next`, plus any `global-monotonic-counter`
   detector findings.
2. `slipstream diff naive optimized --json`; confirm `next`'s write-footprint
   shifts from the shared `Nonce` key to a per-account key and the detector
   finding clears.
3. Synthetic batch of `M` `next()` calls across `A` accounts; have
   slipstream-core compute conflict-graph edges and stages.
4. Report edges/stages (expect naive ~`M` stages, optimized ~`ceil(M / A)`), and
   note the storage-class change (persistent -> temporary) as a qualitative
   state-rent improvement.

## Results

| Metric | Naive | Optimized | Delta |
| --- | --- | --- | --- |
| storage writes (next) | TBD (not yet measured) | TBD | TBD |
| storage class | persistent | temporary | n/a |
| conflict-graph edges (M calls, A accounts) | TBD | TBD | TBD |
| parallel stages (M calls) | TBD | TBD | TBD |
| detector findings | TBD | TBD | TBD |
