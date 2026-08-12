use soroban_sdk::Env;

use crate::{NaiveNonce, NaiveNonceClient};

fn client(env: &Env) -> NaiveNonceClient<'_> {
    let id = env.register(NaiveNonce, ());
    NaiveNonceClient::new(env, &id)
}

#[test]
fn nonces_are_strictly_monotonic() {
    let env = Env::default();
    let c = client(&env);
    assert_eq!(c.current(), 0);
    assert_eq!(c.next(), 1);
    assert_eq!(c.next(), 2);
    assert_eq!(c.next(), 3);
    assert_eq!(c.current(), 3);
}
