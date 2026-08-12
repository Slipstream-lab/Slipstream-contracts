#![no_std]
//! Pattern 03 - Lazy fee accrual (NAIVE variant).
//!
//! Every user-facing operation charges a fee by adding it to a single global
//! accumulator, [`DataKey::FeePool`]. Because each `operate()` writes that one
//! key, every operation across all accounts conflicts on `FeePool`: the fee
//! bookkeeping alone serialises the whole contract under CAP-0063, even when
//! the underlying operations are otherwise independent.
//!
//! The `optimized` crate defers global aggregation ("lazy accrual"): fees are
//! recorded on per-account keys during the hot path and only folded into the
//! global pool when explicitly swept.
//!
//! See `../BENCH.md` for the analysis.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The single global fee accumulator. The contention point.
    FeePool,
    /// Per-account operation count, kept so behaviour matches the optimized
    /// variant. (Independent per account; not the contention point.)
    OpCount(Address),
}

/// Flat fee charged per operation.
pub const FEE_PER_OP: i128 = 5;

#[contract]
pub struct NaiveFee;

#[contractimpl]
impl NaiveFee {
    /// Perform one metered operation on behalf of `who`.
    ///
    /// Write-footprint: `{FeePool, OpCount(who)}`. The `FeePool` write is the
    /// artificial global dependency this pattern is about.
    pub fn operate(env: Env, who: Address) {
        who.require_auth();

        let count: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::OpCount(who.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::OpCount(who), &(count + 1));

        // The global hot key: touched by every operation of every account.
        let pool: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::FeePool)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::FeePool, &(pool + FEE_PER_OP));
    }

    /// Total fees collected so far. Read-footprint: `{FeePool}`.
    pub fn fee_pool(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::FeePool)
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
