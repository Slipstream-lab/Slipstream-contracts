#![no_std]
//! Pattern 02 - Per-user balance (NAIVE variant).
//!
//! All account balances live in a single `Map<Address, i128>` stored under one
//! ledger key, [`DataKey::Balances`]. Any `deposit` or `transfer` must read the
//! whole map, mutate it, and write it back. The write-footprint of *every*
//! balance-changing transaction therefore contains that single `Balances` key,
//! so no two of them can be placed in the same parallel stage under CAP-0063 --
//! Alice depositing and Bob depositing conflict even though they touch
//! logically-independent balances.
//!
//! See `../BENCH.md` for the analysis and the per-account fix in `optimized`.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Map};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The single map holding every account's balance. The contention point.
    Balances,
}

#[contract]
pub struct NaiveBalance;

#[contractimpl]
impl NaiveBalance {
    fn load(env: &Env) -> Map<Address, i128> {
        env.storage()
            .persistent()
            .get(&DataKey::Balances)
            .unwrap_or_else(|| Map::new(env))
    }

    fn store(env: &Env, balances: &Map<Address, i128>) {
        env.storage().persistent().set(&DataKey::Balances, balances);
    }

    /// Credit `amount` to `to`. Write-footprint: `{Balances}` on every call.
    pub fn deposit(env: Env, to: Address, amount: i128) -> i128 {
        let mut balances = Self::load(&env);
        let new_balance = balances.get(to.clone()).unwrap_or(0) + amount;
        balances.set(to, new_balance);
        Self::store(&env, &balances);
        new_balance
    }

    /// Move `amount` from `from` to `to`. Write-footprint: `{Balances}`.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let mut balances = Self::load(&env);
        let from_balance = balances.get(from.clone()).unwrap_or(0);
        assert!(from_balance >= amount, "insufficient balance");
        balances.set(from, from_balance - amount);
        let to_balance = balances.get(to.clone()).unwrap_or(0) + amount;
        balances.set(to, to_balance);
        Self::store(&env, &balances);
    }

    /// Read `who`'s balance. Read-footprint: `{Balances}` (the whole map).
    pub fn balance(env: Env, who: Address) -> i128 {
        Self::load(&env).get(who).unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
