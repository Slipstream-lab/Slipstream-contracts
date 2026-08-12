use soroban_sdk::testutils::storage::Persistent as _;
use soroban_sdk::Env;

use crate::{BatchedAdmin, BatchedAdminClient, Config};

fn client(env: &Env) -> BatchedAdminClient<'_> {
    let id = env.register(BatchedAdmin, ());
    BatchedAdminClient::new(env, &id)
}

fn sample_config() -> Config {
    Config {
        fee_bps: 30,
        max_amount: 1_000_000,
        paused: true,
        admin_count: 3,
        min_amount: 10,
        fee_recipient_set: true,
    }
}

#[test]
fn config_defaults_before_first_write() {
    let env = Env::default();
    let c = client(&env);
    let cfg = c.get_config();
    assert_eq!(cfg.fee_bps, 0);
    assert_eq!(cfg.max_amount, 0);
    assert!(!cfg.paused);
    assert_eq!(cfg.admin_count, 0);
}

#[test]
fn whole_config_is_written_in_one_key() {
    let env = Env::default();
    let c = client(&env);

    let cfg = sample_config();
    c.set_config(&cfg);

    assert_eq!(c.get_config(), cfg);
    assert_eq!(c.fee_bps(), 30);
}

/// The core invariant of this pattern: writing the whole `Config` touches
/// exactly one persistent ledger entry, no matter how many fields the struct
/// carries. This is what keeps the admin transaction's conflict-graph degree
/// constant instead of growing with the config size (unlike the naive variant,
/// which writes one key per field).
#[test]
fn config_write_touches_exactly_one_persistent_key() {
    let env = Env::default();
    let id = env.register(BatchedAdmin, ());
    let c = BatchedAdminClient::new(&env, &id);

    c.set_config(&sample_config());

    // `Config` has six fields, yet only one persistent entry exists.
    let entries = env.as_contract(&id, || env.storage().persistent().all().len());
    assert_eq!(
        entries, 1,
        "set_config must write a single ledger key regardless of field count"
    );
}

/// Overwriting the config does not accumulate keys: a second write still leaves
/// exactly one entry.
#[test]
fn repeated_config_writes_stay_one_key() {
    let env = Env::default();
    let id = env.register(BatchedAdmin, ());
    let c = BatchedAdminClient::new(&env, &id);

    c.set_config(&sample_config());
    let mut updated = sample_config();
    updated.paused = false;
    updated.admin_count = 7;
    c.set_config(&updated);

    let entries = env.as_contract(&id, || env.storage().persistent().all().len());
    assert_eq!(entries, 1);
    assert_eq!(c.get_config(), updated);
}
