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

/// Parameter sweep over the shard count: for each `shards_used` in `[1, 2, 4,
/// 8]`, `writers` distinct writers each increment their own shard in `0..
/// shards_used`. Asserts that the number of disjoint keys touched grows exactly
/// with the parameter, that values stay on their own key (no leakage), and that
/// `total()` still agrees.
///
/// `SHARDS` is a compile-time knob (currently 8); the sweep treats it as the
/// ceiling and verifies the disjoint-key property holds as the shard count grows
/// up to it.
fn shard_sweep_asserts(shards_used: u32, writers: u32) {
    assert!((1..=SHARDS).contains(&shards_used));
    assert!(writers >= 1);
    let env = Env::default();
    let c = client(&env);

    // Writer `w` increments shard `w` exactly `writers` times.
    let mut expected_per_shard = [0u64; SHARDS as usize];
    for w in 0..shards_used {
        for _ in 0..writers {
            c.increment(&w);
            expected_per_shard[w as usize] += 1;
        }
    }

    // Behavioural correctness: each writer's increments landed on its own key.
    for shard in 0..SHARDS {
        assert_eq!(c.shard_total(&shard), expected_per_shard[shard as usize]);
    }
    assert_eq!(c.total(), (shards_used * writers) as u64);

    // Key-disjointness grows with the parameter: exactly `shards_used` distinct
    // keys are nonzero, and no key beyond the parameter is touched at all.
    let distinct_keys = (0..shards_used).filter(|&s| c.shard_total(&s) > 0).count();
    assert_eq!(
        distinct_keys as u32, shards_used,
        "distinct written keys must equal the shard count"
    );
    for shard in shards_used..SHARDS {
        assert_eq!(
            c.shard_total(&shard),
            0,
            "no write may leak past the parameter"
        );
    }
}

#[test]
fn key_disjointness_grows_with_shard_count() {
    for shards_used in [1u32, 2, 4, 8] {
        for writers in [1u32, 4, 8] {
            shard_sweep_asserts(shards_used, writers);
        }
    }
}
