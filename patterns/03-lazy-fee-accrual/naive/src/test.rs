use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{NaiveFee, NaiveFeeClient, FEE_PER_OP};

fn client(env: &Env) -> NaiveFeeClient<'_> {
    let id = env.register(NaiveFee, ());
    NaiveFeeClient::new(env, &id)
}

#[test]
fn every_operation_grows_the_global_pool() {
    let env = Env::default();
    env.mock_all_auths();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    c.operate(&alice);
    c.operate(&bob);
    c.operate(&alice);

    // Three operations across two accounts all hit the one global pool.
    assert_eq!(c.fee_pool(), 3 * FEE_PER_OP);
    assert_eq!(c.op_count(&alice), 2);
    assert_eq!(c.op_count(&bob), 1);
}
