use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{PerUserNonce, PerUserNonceClient};

fn client(env: &Env) -> PerUserNonceClient<'_> {
    let id = env.register(PerUserNonce, ());
    PerUserNonceClient::new(env, &id)
}

#[test]
fn each_account_has_an_independent_nonce_stream() {
    let env = Env::default();
    env.mock_all_auths();
    let c = client(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    assert_eq!(c.next(&alice), 1);
    assert_eq!(c.next(&alice), 2);
    // Bob's stream is independent of Alice's -- starts fresh at 1.
    assert_eq!(c.next(&bob), 1);
    assert_eq!(c.next(&alice), 3);

    assert_eq!(c.current(&alice), 3);
    assert_eq!(c.current(&bob), 1);
}
