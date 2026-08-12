#![no_std]
//! Pattern 05 - Batched admin writes (OPTIMIZED variant).
//!
//! The entire configuration is one [`Config`] struct stored under a single
//! ledger key, [`DataKey::Config`]. `set_config` writes exactly one key
//! regardless of how many fields change, so its write-footprint is `{Config}`
//! -- the narrowest possible. Readers that only need one field still read the
//! whole struct, but reads don't create write-write conflicts, so this trades a
//! slightly larger read for a much smaller, fixed write-footprint.
//!
//! A narrow, fixed footprint is the easiest thing for CAP-0063's scheduler to
//! place, and it keeps the conflict-graph degree of admin transactions
//! constant instead of growing with the number of settings.
//!
//! See `../BENCH.md` for the analysis.

use soroban_sdk::{contract, contractimpl, contracttype, Env};

/// The full contract configuration, versioned so it can evolve in one write.
///
/// Adding a field here is deliberately cheap: because the whole struct lives
/// under the single [`DataKey::Config`] key, a wider `Config` does **not**
/// widen `set_config`'s write-footprint. The `config_stays_one_key_*` tests
/// pin that invariant.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub fee_bps: u32,
    pub max_amount: i128,
    pub paused: bool,
    // Fields added as the contract grows. Each one would be a *separate* ledger
    // key (and conflict-graph vertex) in the naive variant; here they ride
    // along in the single `Config` entry for free.
    pub admin_count: u32,
    pub min_amount: i128,
    pub fee_recipient_set: bool,
}

impl Config {
    fn default(env: &Env) -> Self {
        let _ = env;
        Config {
            fee_bps: 0,
            max_amount: 0,
            paused: false,
            admin_count: 0,
            min_amount: 0,
            fee_recipient_set: false,
        }
    }
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The single key holding the whole config struct.
    Config,
}

#[contract]
pub struct BatchedAdmin;

#[contractimpl]
impl BatchedAdmin {
    /// Replace the entire configuration in one write.
    ///
    /// Write-footprint: `{Config}` -- fixed, no matter how many fields change.
    pub fn set_config(env: Env, config: Config) {
        env.storage().persistent().set(&DataKey::Config, &config);
    }

    /// Read the whole configuration. Read-footprint: `{Config}`.
    pub fn get_config(env: Env) -> Config {
        env.storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| Config::default(&env))
    }

    /// Convenience reader for a single field. Read-footprint: `{Config}`.
    pub fn fee_bps(env: Env) -> u32 {
        Self::get_config(env).fee_bps
    }
}

#[cfg(test)]
mod test;
