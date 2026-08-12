use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{LazyFee, LazyFeeClient, FEE_PER_OP};

fn client(env: &Env) -> LazyFeeClient<'_> {
    let id = env.register(LazyFee, ());
    LazyFeeClient::new(env, &id)
}

#[test]
fn hot_path_does_not_touch_global_pool() {
    let env = Env::default();
    env.mock_all_auths();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    c.operate(&alice);
    c.operate(&bob);
    c.operate(&alice);

    // Fees are accrued locally; the global pool is still empty.
    assert_eq!(c.fee_pool(), 0);
    assert_eq!(c.accrued(&alice), 2 * FEE_PER_OP);
    assert_eq!(c.accrued(&bob), FEE_PER_OP);
    assert_eq!(c.op_count(&alice), 2);
}

#[test]
fn sweep_reconciles_into_global_pool() {
    let env = Env::default();
    env.mock_all_auths();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    c.operate(&alice);
    c.operate(&alice);
    c.operate(&bob);

    assert_eq!(c.sweep(&alice), 2 * FEE_PER_OP);
    assert_eq!(c.accrued(&alice), 0);
    assert_eq!(c.fee_pool(), 2 * FEE_PER_OP);

    // Sweeping bob folds the rest in; the total equals the naive global sum.
    assert_eq!(c.sweep(&bob), 3 * FEE_PER_OP);
    assert_eq!(c.fee_pool(), 3 * FEE_PER_OP);
}
