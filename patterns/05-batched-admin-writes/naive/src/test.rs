use soroban_sdk::{vec, Env};

use crate::{NaiveAdmin, NaiveAdminClient};

fn client(env: &Env) -> NaiveAdminClient<'_> {
    let id = env.register(NaiveAdmin, ());
    NaiveAdminClient::new(env, &id)
}

#[test]
fn each_setting_is_stored_under_its_own_key() {
    let env = Env::default();
    let c = client(&env);

    c.set_config(&vec![&env, 10, 20, 30]);

    assert_eq!(c.get_setting(&0), 10);
    assert_eq!(c.get_setting(&1), 20);
    assert_eq!(c.get_setting(&2), 30);
    // Unset field reads as zero.
    assert_eq!(c.get_setting(&3), 0);
}
