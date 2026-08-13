//! Detector conformance: pin the detectors `slipstream scan --json` reports for
//! each *naive* contract in the corpus.
//!
//! The naive contracts are known-positive fixtures for the contention
//! anti-patterns, so their detector output should be stable. This test runs a
//! real `slipstream` binary (path from `SLIPSTREAM_BIN`, default `slipstream`
//! on `PATH`) over every `patterns/NN-*/naive` directory and asserts the exact
//! set of detector names reported.
//!
//! The test **skips cleanly** (it does not fail) when the binary is absent:
//! `slipstream-core` is not vendored in this repo, so CI without `SLIPSTREAM_BIN`
//! runs nothing here.

use std::path::{Path, PathBuf};

use harness::{discover_patterns, model::AnalysisReport, CoreRunner, Pattern, SubprocessRunner};

/// The expected detector findings per naive contract.
///
/// This is the single source of truth for the detector pinning; the same table
/// is documented in `docs/PLAYBOOK.md` ("Expected detector findings"). Update
/// both when a detector evolves.
const EXPECTED_DETECTORS: &[(&str, &[&str])] = &[
    ("01-sharded-counter", &["read-modify-write"]),
    // 02-naive keeps the whole `Balances` map under one key, but that
    // false-sharing is not statically detectable as a single key today, so the
    // analyzer (correctly, per its current detectors) reports nothing.
    ("02-per-user-balance", &[]),
    ("03-lazy-fee-accrual", &["read-modify-write"]),
    ("04-temporary-nonce", &["read-modify-write"]),
    ("05-batched-admin-writes", &["write-in-loop"]),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn bin() -> Option<SubprocessRunner> {
    let runner = match std::env::var("SLIPSTREAM_BIN") {
        Ok(bin) => SubprocessRunner::new(bin),
        Err(_) => SubprocessRunner::new("slipstream"),
    };
    if runner.available() {
        Some(runner)
    } else {
        None
    }
}

fn detector_names(runner: &dyn CoreRunner, naive: &std::path::Path) -> Vec<String> {
    let raw = runner.scan(naive).expect("slipstream scan succeeds");
    let reports: Vec<AnalysisReport> = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("scan output for {} parses: {e}", naive.display()));
    let mut names: Vec<String> = reports
        .iter()
        .flat_map(|r| r.detectors.iter().map(|d| d.detector.clone()))
        .collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn naive_contracts_trip_the_pinned_detectors() {
    let Some(runner) = bin() else {
        eprintln!(
            "SKIP detector conformance: `slipstream` binary not found; \
             set SLIPSTREAM_BIN to a slipstream-core binary to run."
        );
        return;
    };

    let patterns = discover_patterns(&repo_root().join("patterns"));
    assert!(!patterns.is_empty(), "pattern corpus present");
    let naive_only: Vec<&Pattern> = patterns.iter().filter(|p| p.naive.is_some()).collect();
    assert!(
        !naive_only.is_empty(),
        "at least one pattern has a naive side"
    );

    for p in naive_only {
        let naive = p.naive.as_ref().expect("filtered for naive side");
        let expected: Vec<&str> = EXPECTED_DETECTORS
            .iter()
            .find(|(name, _)| *name == p.name)
            .map(|(_, detectors)| detectors.to_vec())
            .unwrap_or_else(|| panic!("expected-detectors table missing entry for {}", p.name));
        let actual = detector_names(&runner, naive);
        assert_eq!(actual, expected, "detector findings for naive {}", p.name);
    }
}
