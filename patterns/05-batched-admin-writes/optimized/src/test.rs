use soroban_sdk::Env;

use crate::{BatchedAdmin, BatchedAdminClient, Config};

fn client(env: &Env) -> BatchedAdminClient<'_> {
    let id = env.register(BatchedAdmin, ());
    BatchedAdminClient::new(env, &id)
}

#[test]
fn config_defaults_before_first_write() {
    let env = Env::default();
    let c = client(&env);
    let cfg = c.get_config();
    assert_eq!(cfg.fee_bps, 0);
    assert_eq!(cfg.max_amount, 0);
    assert!(!cfg.paused);
}

#[test]
fn whole_config_is_written_in_one_key() {
    let env = Env::default();
    let c = client(&env);

    let cfg = Config {
        fee_bps: 30,
        max_amount: 1_000_000,
        paused: true,
    };
    c.set_config(&cfg);

    assert_eq!(c.get_config(), cfg);
    assert_eq!(c.fee_bps(), 30);
}
