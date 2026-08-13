use soroban_sdk::{Env, String};

use crate::{NaiveEventLog, NaiveEventLogClient};

fn client(env: &Env) -> NaiveEventLogClient<'_> {
    let id = env.register(NaiveEventLog, ());
    NaiveEventLogClient::new(env, &id)
}

fn msg(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

#[test]
fn appends_are_sequenced_by_the_shared_tail() {
    let env = Env::default();
    let c = client(&env);
    assert_eq!(c.entry_count(), 0);

    assert_eq!(c.append(&msg(&env, "a")), 0);
    assert_eq!(c.append(&msg(&env, "b")), 1);
    assert_eq!(c.append(&msg(&env, "c")), 2);
    assert_eq!(c.entry_count(), 3);

    assert_eq!(c.get(&0), Some(msg(&env, "a")));
    assert_eq!(c.get(&2), Some(msg(&env, "c")));
    assert_eq!(c.get(&3), None);
}

#[test]
fn every_append_bumps_the_single_tail() {
    let env = Env::default();
    let c = client(&env);
    let mut expected = 0u64;
    for i in 0..25u64 {
        assert_eq!(c.append(&msg(&env, "x")), i);
        expected += 1;
    }
    assert_eq!(c.entry_count(), expected);
}
