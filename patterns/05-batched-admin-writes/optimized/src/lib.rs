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
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub fee_bps: u32,
    pub max_amount: i128,
    pub paused: bool,
    // TODO: additional config fields land here as the contract grows. Because
    // the whole struct is one ledger key, adding a field does NOT widen the
    // write-footprint of `set_config` -- that is the whole point of the pattern.
}

impl Config {
    fn default(env: &Env) -> Self {
        let _ = env;
        Config {
            fee_bps: 0,
            max_amount: 0,
            paused: false,
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
