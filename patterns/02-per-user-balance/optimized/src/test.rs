use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{PerUserBalance, PerUserBalanceClient};

fn client(env: &Env) -> PerUserBalanceClient<'_> {
    let id = env.register(PerUserBalance, ());
    PerUserBalanceClient::new(env, &id)
}

#[test]
fn independent_accounts_have_independent_keys() {
    let env = Env::default();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    c.deposit(&alice, &100);
    c.deposit(&bob, &7);

    assert_eq!(c.balance(&alice), 100);
    assert_eq!(c.balance(&bob), 7);
    // An untouched account reads as zero without ever having a key written.
    assert_eq!(c.balance(&Address::generate(&env)), 0);
}

#[test]
fn transfer_touches_both_endpoints() {
    let env = Env::default();
    env.mock_all_auths();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    c.deposit(&alice, &100);
    c.transfer(&alice, &bob, &30);

    assert_eq!(c.balance(&alice), 70);
    assert_eq!(c.balance(&bob), 30);
}
