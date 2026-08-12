#![no_std]
//! Pattern 05 - Batched admin writes (NAIVE variant).
//!
//! Reconfiguring the contract writes each config field to its own ledger key in
//! a loop: `set_config` touches `{Setting(0), Setting(1), ..., Setting(n-1)}`.
//! That wide write-footprint inflates the conflict graph -- every concurrent
//! transaction that reads *any* config field now has an edge to the admin
//! transaction, and admin transactions maximally overlap each other. Under
//! CAP-0063 a wide footprint is harder to schedule into a parallel stage than a
//! narrow one, because it collides with more of the batch.
//!
//! The `optimized` crate collapses the whole config into one struct under a
//! single key. See `../BENCH.md`.

use soroban_sdk::{contract, contractimpl, contracttype, Env, Vec};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// One ledger key per config field, indexed by field number.
    Setting(u32),
}

#[contract]
pub struct NaiveAdmin;

#[contractimpl]
impl NaiveAdmin {
    /// Write every provided setting to its own key, one at a time.
    ///
    /// Write-footprint: `{Setting(0), .., Setting(len-1)}` -- grows with the
    /// number of fields.
    pub fn set_config(env: Env, values: Vec<i128>) {
        for (i, value) in values.iter().enumerate() {
            env.storage()
                .persistent()
                .set(&DataKey::Setting(i as u32), &value);
        }
    }

    /// Read a single setting. Read-footprint: `{Setting(index)}`.
    pub fn get_setting(env: Env, index: u32) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Setting(index))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
