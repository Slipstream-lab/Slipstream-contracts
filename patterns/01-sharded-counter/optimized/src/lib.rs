#![no_std]
//! Pattern 01 - Sharded counter (OPTIMIZED variant).
//!
//! Instead of a single global key, the count is spread across `SHARDS`
//! independent persistent ledger keys `("shard", i)`. A writer picks a shard
//! and only ever touches that one key, so its write-footprint is
//! `{("shard", i)}`. Two increments that target *different* shards have
//! disjoint write-footprints and can therefore be scheduled into the same
//! parallel stage under CAP-0063 without conflicting.
//!
//! `total()` reads every shard and sums them; it is the (rare) read-side
//! aggregation that pays for the cheap, contention-free writes.
//!
//! See `../BENCH.md` for the full contention analysis.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Env};

/// Number of shards the counter is spread across.
///
/// This is the tuning knob: more shards means less write contention (more
/// increments can run in parallel) at the cost of a more expensive `total()`.
pub const SHARDS: u32 = 8;

/// Ledger keys used by the sharded counter.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// One counter per shard index. Each is an independent ledger key.
    Shard(u32),
}

/// Errors returned by the sharded counter.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// The provided shard index was `>= SHARDS`.
    ShardOutOfRange = 1,
}

#[contract]
pub struct ShardedCounter;

#[contractimpl]
impl ShardedCounter {
    /// Increment the counter on a single shard. Returns that shard's new value.
    ///
    /// Write-footprint: `{("shard", shard)}` only. Callers should spread their
    /// traffic across shards (e.g. by hashing an account id modulo [`SHARDS`])
    /// to avoid re-creating a hot key.
    pub fn increment(env: Env, shard: u32) -> Result<u64, Error> {
        if shard >= SHARDS {
            return Err(Error::ShardOutOfRange);
        }
        let key = DataKey::Shard(shard);
        let mut value: u64 = env.storage().persistent().get(&key).unwrap_or(0);
        value += 1;
        env.storage().persistent().set(&key, &value);
        Ok(value)
    }

    /// Read a single shard's value. Read-footprint: `{("shard", shard)}`.
    pub fn shard_total(env: Env, shard: u32) -> Result<u64, Error> {
        if shard >= SHARDS {
            return Err(Error::ShardOutOfRange);
        }
        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::Shard(shard))
            .unwrap_or(0))
    }

    /// Sum every shard. Read-footprint: `{("shard", 0), .., ("shard", N-1)}`.
    ///
    /// This is the price of the design: aggregation touches all shard keys, so
    /// it conflicts with every writer. It is expected to be called rarely
    /// (reporting) relative to `increment` (the hot path).
    pub fn total(env: Env) -> u64 {
        let mut sum: u64 = 0;
        for shard in 0..SHARDS {
            let value: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::Shard(shard))
                .unwrap_or(0);
            sum += value;
        }
        sum
    }

    /// Number of shards this contract is configured with.
    pub fn shards(_env: Env) -> u32 {
        SHARDS
    }
}

#[cfg(test)]
mod test;
