#![no_std]
//! Pattern 04 - Temporary nonce (OPTIMIZED variant).
//!
//! Each account gets its own nonce, stored in *temporary* storage under
//! [`DataKey::Nonce`]`(addr)`. A `next(who)` call touches only
//! `{Nonce(who)}`, so different accounts requesting nonces have disjoint
//! footprints and parallelise under CAP-0063. Using temporary storage also
//! means the key is cheaply evicted once it stops being touched, so
//! replay-protection state does not accumulate persistent ledger rent.
//!
//! Nonces are only guaranteed monotonic *per account*, which is exactly the
//! property replay protection needs.
//!
//! See `../BENCH.md` for the analysis.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Per-account nonce held in temporary storage. Independent per account.
    Nonce(Address),
}

#[contract]
pub struct PerUserNonce;

#[contractimpl]
impl PerUserNonce {
    /// Hand out `who`'s next nonce and advance their personal counter.
    ///
    /// Write-footprint: `{Nonce(who)}` -- private to the account.
    pub fn next(env: Env, who: Address) -> u64 {
        who.require_auth();
        let key = DataKey::Nonce(who);
        let current: u64 = env.storage().temporary().get(&key).unwrap_or(0);
        let next = current + 1;
        env.storage().temporary().set(&key, &next);
        next
    }

    /// Current value of `who`'s nonce. Read-footprint: `{Nonce(who)}`.
    pub fn current(env: Env, who: Address) -> u64 {
        env.storage()
            .temporary()
            .get(&DataKey::Nonce(who))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
