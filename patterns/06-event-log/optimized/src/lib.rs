#![no_std]
//! Pattern 06 - Event log (OPTIMIZED variant).
//!
//! The log is split into [`SEGMENTS`] independent per-writer segments. Each
//! segment has its own [`DataKey::SegmentTail`] pointer, and entries live under
//! [`DataKey::Entry`]`(segment, index)`. An `append(segment, msg)` reads and
//! writes only that segment's tail and entry, so appends to *different*
//! segments have disjoint footprints and can be scheduled into the same
//! parallel stage under CAP-0063 without conflicting.
//!
//! Appends within one segment still serialize (as they should -- they are an
//! ordered per-writer stream). The log's read side (`total_len`, `get`) can
//! still read across segments, at the price of touching all segment tails.
//!
//! See `../BENCH.md` for the analysis.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Env, String};

/// Number of independent writer segments. The tuning knob: more segments means
/// more concurrent appenders can run in parallel, at the cost of a wider
/// read-side aggregation.
pub const SEGMENTS: u32 = 8;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Per-segment tail pointer: entries appended to that segment so far.
    /// Private to the segment.
    SegmentTail(u32),
    /// Entry at `(segment, index)`. Both are runtime values, so the analyzer
    /// sees these as `(dynamic)`.
    Entry(u32, u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// The provided segment index was `>= SEGMENTS`.
    SegmentOutOfRange = 1,
}

#[contract]
pub struct SegmentedEventLog;

#[contractimpl]
impl SegmentedEventLog {
    /// Append `msg` to `segment`. Returns the index within that segment.
    ///
    /// Write-footprint: `{SegmentTail(segment), Entry(segment, tail)}` only.
    /// Appends to distinct segments touch disjoint keys and do not conflict.
    pub fn append(env: Env, segment: u32, msg: String) -> Result<u64, Error> {
        if segment >= SEGMENTS {
            return Err(Error::SegmentOutOfRange);
        }
        let tail: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::SegmentTail(segment))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Entry(segment, tail), &msg);
        let next = tail + 1;
        env.storage()
            .persistent()
            .set(&DataKey::SegmentTail(segment), &next);
        Ok(tail)
    }

    /// Number of entries in `segment`. Read-footprint: `{SegmentTail(segment)}`.
    pub fn segment_len(env: Env, segment: u32) -> Result<u64, Error> {
        if segment >= SEGMENTS {
            return Err(Error::SegmentOutOfRange);
        }
        Ok(env
            .storage()
            .persistent()
            .get(&DataKey::SegmentTail(segment))
            .unwrap_or(0))
    }

    /// Total entries across every segment. Read-footprint: every
    /// `SegmentTail` -- the (rare) aggregation cost of the design.
    pub fn total_len(env: Env) -> u64 {
        let mut sum: u64 = 0;
        for segment in 0..SEGMENTS {
            sum += env
                .storage()
                .persistent()
                .get(&DataKey::SegmentTail(segment))
                .unwrap_or(0);
        }
        sum
    }

    /// Read the entry at `(segment, index)`, if it exists.
    pub fn get(env: Env, segment: u32, index: u64) -> Option<String> {
        env.storage()
            .persistent()
            .get(&DataKey::Entry(segment, index))
    }

    /// Number of segments this contract is configured with.
    pub fn segments(_env: Env) -> u32 {
        SEGMENTS
    }
}

#[cfg(test)]
mod test;
