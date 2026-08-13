use soroban_sdk::{Env, String};

use crate::{Error, SegmentedEventLog, SegmentedEventLogClient, SEGMENTS};

fn client(env: &Env) -> SegmentedEventLogClient<'_> {
    let id = env.register(SegmentedEventLog, ());
    SegmentedEventLogClient::new(env, &id)
}

fn msg(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

#[test]
fn appends_are_sequenced_per_segment() {
    let env = Env::default();
    let c = client(&env);
    assert_eq!(c.total_len(), 0);

    assert_eq!(c.append(&0, &msg(&env, "a")), 0);
    assert_eq!(c.append(&0, &msg(&env, "b")), 1);
    // Segment 1 starts its own, independent stream.
    assert_eq!(c.append(&1, &msg(&env, "x")), 0);
    assert_eq!(c.append(&0, &msg(&env, "c")), 2);

    assert_eq!(c.segment_len(&0), 3);
    assert_eq!(c.segment_len(&1), 1);
    assert_eq!(c.total_len(), 4);

    assert_eq!(c.get(&0, &0), Some(msg(&env, "a")));
    assert_eq!(c.get(&1, &0), Some(msg(&env, "x")));
    assert_eq!(c.get(&0, &3), None);
}

#[test]
fn distinct_segments_are_independent() {
    let env = Env::default();
    let c = client(&env);
    for i in 0..10u64 {
        assert_eq!(c.append(&0, &msg(&env, "s0")), i);
    }
    for i in 0..5u64 {
        assert_eq!(c.append(&7, &msg(&env, "s7")), i);
    }
    // Nonzero exactly on the segments written: no leakage across segments.
    assert_eq!(c.segment_len(&0), 10);
    assert_eq!(c.segment_len(&7), 5);
    assert_eq!(c.segment_len(&3), 0);
    assert_eq!(c.total_len(), 15);
}

#[test]
fn out_of_range_segment_is_rejected() {
    let env = Env::default();
    let c = client(&env);
    assert_eq!(
        c.try_append(&SEGMENTS, &msg(&env, "x")),
        Err(Ok(Error::SegmentOutOfRange))
    );
    assert_eq!(
        c.try_segment_len(&SEGMENTS),
        Err(Ok(Error::SegmentOutOfRange))
    );
}
