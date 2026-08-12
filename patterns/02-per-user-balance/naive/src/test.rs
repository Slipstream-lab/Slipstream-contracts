use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{NaiveBalance, NaiveBalanceClient};

fn client(env: &Env) -> NaiveBalanceClient<'_> {
    let id = env.register(NaiveBalance, ());
    NaiveBalanceClient::new(env, &id)
}

#[test]
fn deposits_accumulate_per_account() {
    let env = Env::default();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    assert_eq!(c.deposit(&alice, &100), 100);
    assert_eq!(c.deposit(&alice, &50), 150);
    assert_eq!(c.deposit(&bob, &10), 10);

    assert_eq!(c.balance(&alice), 150);
    assert_eq!(c.balance(&bob), 10);
}

#[test]
fn transfer_moves_funds() {
    let env = Env::default();
    env.mock_all_auths();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    c.deposit(&alice, &100);
    c.transfer(&alice, &bob, &40);

    assert_eq!(c.balance(&alice), 60);
    assert_eq!(c.balance(&bob), 40);
}
