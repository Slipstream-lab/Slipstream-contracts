use soroban_sdk::Env;

use crate::{NaiveCounter, NaiveCounterClient};

fn client(env: &Env) -> NaiveCounterClient<'_> {
    let id = env.register(NaiveCounter, ());
    NaiveCounterClient::new(env, &id)
}

#[test]
fn counts_from_zero() {
    let env = Env::default();
    let c = client(&env);
    assert_eq!(c.total(), 0);
    assert_eq!(c.increment(), 1);
    assert_eq!(c.increment(), 2);
    assert_eq!(c.increment(), 3);
    assert_eq!(c.total(), 3);
}

#[test]
fn total_matches_number_of_increments() {
    let env = Env::default();
    let c = client(&env);
    for _ in 0..25 {
        c.increment();
    }
    assert_eq!(c.total(), 25);
}
