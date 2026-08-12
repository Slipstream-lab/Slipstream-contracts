use soroban_sdk::Env;

use crate::{Error, ShardedCounter, ShardedCounterClient, SHARDS};

fn client(env: &Env) -> ShardedCounterClient<'_> {
    let id = env.register(ShardedCounter, ());
    ShardedCounterClient::new(env, &id)
}

#[test]
fn total_equals_sum_of_shards() {
    let env = Env::default();
    let c = client(&env);
    assert_eq!(c.total(), 0);

    // Spread 100 increments deterministically across all shards.
    let mut expected_per_shard = [0u64; SHARDS as usize];
    for i in 0..100u32 {
        let shard = i % SHARDS;
        c.increment(&shard);
        expected_per_shard[shard as usize] += 1;
    }

    for shard in 0..SHARDS {
        assert_eq!(c.shard_total(&shard), expected_per_shard[shard as usize]);
    }
    assert_eq!(c.total(), 100);
    assert_eq!(c.shards(), SHARDS);
}

#[test]
fn independent_shards_do_not_interfere() {
    let env = Env::default();
    let c = client(&env);
    c.increment(&0);
    c.increment(&0);
    c.increment(&7);
    assert_eq!(c.shard_total(&0), 2);
    assert_eq!(c.shard_total(&7), 1);
    assert_eq!(c.shard_total(&3), 0);
    assert_eq!(c.total(), 3);
}

#[test]
fn out_of_range_shard_is_rejected() {
    let env = Env::default();
    let c = client(&env);
    assert_eq!(c.try_increment(&SHARDS), Err(Ok(Error::ShardOutOfRange)));
    assert_eq!(c.try_shard_total(&SHARDS), Err(Ok(Error::ShardOutOfRange)));
}
