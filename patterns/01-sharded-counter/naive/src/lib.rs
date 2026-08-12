#![no_std]
//! Pattern 01 - Sharded counter (NAIVE variant).
//!
//! Every `increment()` reads and writes a single persistent ledger key,
//! [`DataKey::Counter`]. That single key is present in the write-footprint of
//! *every* transaction that touches this contract, so no two increment
//! transactions can ever be scheduled into the same parallel stage under
//! Stellar's phased execution (CAP-0063): they always conflict on that key.
//!
//! See `../BENCH.md` for the contention analysis and the fix in the
//! `optimized` crate.

use soroban_sdk::{contract, contractimpl, contracttype, Env};

/// Ledger keys used by the naive counter.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The one and only counter. This is the contention point.
    Counter,
}

#[contract]
pub struct NaiveCounter;

#[contractimpl]
impl NaiveCounter {
    /// Read the global counter, add one, write it back. Returns the new value.
    ///
    /// Write-footprint: `{Counter}` on every call.
    pub fn increment(env: Env) -> u64 {
        let mut value: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0);
        value += 1;
        env.storage().persistent().set(&DataKey::Counter, &value);
        value
    }

    /// Read the current global counter value. Read-footprint: `{Counter}`.
    pub fn total(env: Env) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
