#![no_std]
//! Pattern 04 - Temporary nonce (NAIVE variant).
//!
//! A single persistent monotonic counter, [`DataKey::Nonce`], is bumped on
//! every `next()` call to hand out replay-protection nonces. Because every
//! caller reads and writes that one persistent key, all nonce requests conflict
//! on it under CAP-0063 -- a global serialisation point purely for uniqueness.
//! Being *persistent*, the key also occupies ledger state forever and pays
//! rent, even though a nonce only needs to be unique among an account's
//! in-flight transactions.
//!
//! The `optimized` crate gives each account its own nonce in *temporary*
//! storage. See `../BENCH.md`.

use soroban_sdk::{contract, contractimpl, contracttype, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The single global monotonic nonce. The contention point.
    Nonce,
}

#[contract]
pub struct NaiveNonce;

#[contractimpl]
impl NaiveNonce {
    /// Hand out the next nonce and advance the global counter.
    ///
    /// Write-footprint: `{Nonce}` on every call, from every account.
    pub fn next(env: Env) -> u64 {
        let current: u64 = env.storage().persistent().get(&DataKey::Nonce).unwrap_or(0);
        let next = current + 1;
        env.storage().persistent().set(&DataKey::Nonce, &next);
        next
    }

    /// Current value of the global nonce. Read-footprint: `{Nonce}`.
    pub fn current(env: Env) -> u64 {
        env.storage().persistent().get(&DataKey::Nonce).unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
