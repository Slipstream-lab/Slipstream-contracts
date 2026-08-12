#![no_std]
//! Pattern 03 - Lazy fee accrual (OPTIMIZED variant).
//!
//! Fees are accrued *lazily* on per-account keys. The hot path, `operate()`,
//! only touches `{OpCount(who), AccruedFee(who)}` -- keys private to that
//! account -- so operations by different accounts have disjoint footprints and
//! parallelise under CAP-0063.
//!
//! The global total is materialised only when an operator calls `sweep(who)`,
//! which moves one account's accrued fees into the global [`DataKey::FeePool`].
//! Sweeps still conflict on `FeePool`, but they are administrative and rare
//! compared to `operate`, so the hot path stays contention-free.
//!
//! This is the ledger analogue of "lazy accrual" in accounting systems: record
//! locally, reconcile globally on demand.
//!
//! See `../BENCH.md` for the analysis.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Per-account operation count. Independent per account.
    OpCount(Address),
    /// Per-account fees not yet swept into the global pool. Independent per
    /// account -- this is what keeps the hot path parallel.
    AccruedFee(Address),
    /// Global pool, touched only by `sweep` (the cold path).
    FeePool,
}

/// Flat fee charged per operation. Matches the naive variant.
pub const FEE_PER_OP: i128 = 5;

#[contract]
pub struct LazyFee;

#[contractimpl]
impl LazyFee {
    /// Perform one metered operation on behalf of `who`.
    ///
    /// Write-footprint: `{OpCount(who), AccruedFee(who)}` -- no global key.
    pub fn operate(env: Env, who: Address) {
        who.require_auth();

        let count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::OpCount(who.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::OpCount(who.clone()), &(count + 1));

        let accrued: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::AccruedFee(who.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::AccruedFee(who), &(accrued + FEE_PER_OP));
    }

    /// Fold `who`'s accrued fees into the global pool and reset their accrual.
    ///
    /// Write-footprint: `{AccruedFee(who), FeePool}`. This is the cold path:
    /// sweeps serialise on `FeePool`, but they are expected to be infrequent.
    pub fn sweep(env: Env, who: Address) -> i128 {
        let accrued: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::AccruedFee(who.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::AccruedFee(who), &0i128);

        let pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::FeePool)
            .unwrap_or(0);
        let new_pool = pool + accrued;
        env.storage().persistent().set(&DataKey::FeePool, &new_pool);
        new_pool
    }

    /// Fees swept into the global pool so far. Read-footprint: `{FeePool}`.
    pub fn fee_pool(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::FeePool)
            .unwrap_or(0)
    }

    /// Fees accrued but not yet swept for `who`.
    /// Read-footprint: `{AccruedFee(who)}`.
    pub fn accrued(env: Env, who: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::AccruedFee(who))
            .unwrap_or(0)
    }

    /// Number of operations performed by `who`. Read-footprint: `{OpCount(who)}`.
    pub fn op_count(env: Env, who: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::OpCount(who))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
