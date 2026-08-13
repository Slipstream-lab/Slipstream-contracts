#![no_std]
//! Pattern 06 - Event log (NAIVE variant).
//!
//! An append-only log. Every `append()` reads the single shared [`DataKey::Tail`]
//! pointer, writes the new entry under [`DataKey::Entry`]`(tail)`, and bumps
//! `Tail`. Because every append from every writer reads and writes that one
//! persistent key, all appends conflict on it under CAP-0063 and serialise --
//! a global bottleneck imposed on otherwise independent log writes.
//!
//! The `optimized` crate splits the log into per-writer segments, each with its
//! own tail pointer, so concurrent appends touch disjoint keys. See
//! `../BENCH.md`.

use soroban_sdk::{contract, contractimpl, contracttype, Env, String};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The single global tail pointer: number of entries appended so far.
    /// Read and written by every append. The contention point.
    Tail,
    /// The log entry at a given index. Unresolvable at compile time (the index
    /// is only known at runtime), so the analyzer sees these as `(dynamic)`.
    Entry(u64),
}

#[contract]
pub struct NaiveEventLog;

#[contractimpl]
impl NaiveEventLog {
    /// Append `msg` to the log. Returns the index the entry was stored at.
    ///
    /// Write-footprint: `{Tail, Entry(tail)}` on every call. The shared `Tail`
    /// read-modify-write serialises every append, from every writer.
    pub fn append(env: Env, msg: String) -> u64 {
        let tail: u64 = env.storage().persistent().get(&DataKey::Tail).unwrap_or(0);
        env.storage().persistent().set(&DataKey::Entry(tail), &msg);
        let next = tail + 1;
        env.storage().persistent().set(&DataKey::Tail, &next);
        tail
    }

    /// Number of entries in the log. Read-footprint: `{Tail}`.
    pub fn entry_count(env: Env) -> u64 {
        env.storage().persistent().get(&DataKey::Tail).unwrap_or(0)
    }

    /// Read the entry at `index`, if it exists. Read-footprint: `{Entry(i)}`.
    pub fn get(env: Env, index: u64) -> Option<String> {
        env.storage().persistent().get(&DataKey::Entry(index))
    }
}

#[cfg(test)]
mod test;
