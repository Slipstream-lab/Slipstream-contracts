#![no_std]
//! Pattern 02 - Per-user balance (OPTIMIZED variant).
//!
//! Each account's balance is stored under its own ledger key,
//! [`DataKey::Balance`]`(addr)`. A `deposit` to Alice touches only
//! `{Balance(alice)}`; a `deposit` to Bob touches only `{Balance(bob)}`. Those
//! footprints are disjoint, so the two transactions can run in the same
//! parallel stage under CAP-0063.
//!
//! A `transfer(from, to)` necessarily touches `{Balance(from), Balance(to)}`,
//! so transfers only conflict when they share an endpoint -- which is the
//! true, unavoidable data dependency rather than an artificial one.
//!
//! See `../BENCH.md` for the analysis.

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// One independent ledger key per account.
    Balance(Address),
}

#[contract]
pub struct PerUserBalance;

#[contractimpl]
impl PerUserBalance {
    fn read(env: &Env, who: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(who.clone()))
            .unwrap_or(0)
    }

    fn write(env: &Env, who: &Address, amount: i128) {
        env.storage()
            .persistent()
            .set(&DataKey::Balance(who.clone()), &amount);
    }

    /// Credit `amount` to `to`. Write-footprint: `{Balance(to)}` only.
    pub fn deposit(env: Env, to: Address, amount: i128) -> i128 {
        let new_balance = Self::read(&env, &to) + amount;
        Self::write(&env, &to, new_balance);
        new_balance
    }

    /// Move `amount` from `from` to `to`.
    /// Write-footprint: `{Balance(from), Balance(to)}` -- the real dependency.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let from_balance = Self::read(&env, &from);
        assert!(from_balance >= amount, "insufficient balance");
        Self::write(&env, &from, from_balance - amount);
        let to_balance = Self::read(&env, &to) + amount;
        Self::write(&env, &to, to_balance);
    }

    /// Read `who`'s balance. Read-footprint: `{Balance(who)}` only.
    pub fn balance(env: Env, who: Address) -> i128 {
        Self::read(&env, &who)
    }
}

#[cfg(test)]
mod test;
