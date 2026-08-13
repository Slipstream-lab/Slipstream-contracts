use std::path::{Path, PathBuf};

use crate::{
    analyze_pair, contention_delta, discover_patterns, model::DiffReport, render_bench_block,
    render_table, run_all, signed_delta, update_bench_md, BenchProvenance, ContentionDelta,
    CoreRunner, HarnessError, MockRunner, Pattern, PatternResult,
};

/// A representative `slipstream diff --json` payload for pattern 01: the
/// optimized (sharded) variant does one fewer contended write and clears the
/// single detector finding the naive variant tripped.
const SAMPLE_DIFF: &str = r#"
{
  "left": {
    "path": "patterns/01-sharded-counter/naive",
    "files": 1,
    "functions": 2,
    "storage_reads": 2,
    "storage_writes": 1,
    "detector_findings": 1,
    "detectors": { "global-write-hotkey": 1 }
  },
  "right": {
    "path": "patterns/01-sharded-counter/optimized",
    "files": 1,
    "functions": 4,
    "storage_reads": 10,
    "storage_writes": 1,
    "detector_findings": 0,
    "detectors": {}
  },
  "per_function_deltas": [
    { "function": "increment", "reads_delta": 0, "writes_delta": 0 },
    { "function": "total", "reads_delta": 7, "writes_delta": 0 }
  ],
  "summary": {
    "detector_findings_delta": -1,
    "storage_reads_delta": 8,
    "storage_writes_delta": -1
  }
}
"#;

#[test]
fn parses_diff_report() {
    let report: DiffReport = serde_json::from_str(SAMPLE_DIFF).unwrap();
    assert_eq!(report.left.detectors.get("global-write-hotkey"), Some(&1));
    assert_eq!(report.right.detector_findings, 0);
    assert_eq!(report.per_function_deltas.len(), 2);
}

#[test]
fn computes_contention_delta_from_report() {
    let report: DiffReport = serde_json::from_str(SAMPLE_DIFF).unwrap();
    let delta = contention_delta(&report);
    assert_eq!(delta.storage_writes_delta, -1);
    assert_eq!(delta.storage_reads_delta, 8);
    assert_eq!(delta.detector_findings_delta, -1);
    // Fewer contended writes + fewer findings == an improvement, even though
    // the optimized variant does more (read-side) work in `total`.
    assert!(delta.is_improvement());
}

#[test]
fn analyze_pair_uses_the_runner_and_parses() {
    let runner = MockRunner::with_diff(SAMPLE_DIFF);
    let delta = analyze_pair(
        &runner,
        Path::new("patterns/01-sharded-counter/naive"),
        Path::new("patterns/01-sharded-counter/optimized"),
    )
    .expect("mock diff should parse");
    assert_eq!(delta.detector_findings_delta, -1);
    assert!(delta.is_improvement());
}

#[test]
fn non_improving_delta_is_flagged() {
    // A degenerate diff where nothing improved: no fewer writes.
    let json = r#"
    {
      "left":  { "storage_writes": 1, "detector_findings": 1 },
      "right": { "storage_writes": 1, "detector_findings": 1 },
      "per_function_deltas": [],
      "summary": { "detector_findings_delta": 0, "storage_reads_delta": 0, "storage_writes_delta": 0 }
    }"#;
    let report: DiffReport = serde_json::from_str(json).unwrap();
    assert!(!contention_delta(&report).is_improvement());
}

#[test]
fn parse_error_surfaces_as_harness_error() {
    let runner = MockRunner::with_diff("not json at all");
    let err = analyze_pair(&runner, Path::new("a"), Path::new("b")).unwrap_err();
    assert!(matches!(err, crate::HarnessError::Parse(_)));
}

/// A real `slipstream scan --json` array element, copied from the engine's
/// output. Guards the `AnalysisReport` model against the actual contract:
/// `source_name` (not `source`), `StaticKey { segments }` storage keys, and
/// nullable `function`/`key` on findings.
const SAMPLE_SCAN: &str = r#"
[
  {
    "source_name": "patterns/01-sharded-counter/naive/src/lib.rs",
    "functions": [
      {
        "function_name": "increment",
        "storage_reads":  [ { "segments": ["Counter"] } ],
        "storage_writes": [ { "segments": ["Counter"] } ]
      },
      {
        "function_name": "write_all",
        "storage_reads":  [],
        "storage_writes": [ { "segments": [] } ]
      }
    ],
    "detectors": [
      {
        "detector": "read-modify-write",
        "function": "increment",
        "key": "Counter",
        "message": "reads and writes the same key"
      },
      {
        "detector": "global-static-write",
        "function": null,
        "key": "Counter",
        "message": "written from multiple functions"
      }
    ]
  }
]
"#;

#[test]
fn parses_real_scan_report() {
    use crate::model::AnalysisReport;
    let reports: Vec<AnalysisReport> = serde_json::from_str(SAMPLE_SCAN).unwrap();
    let report = &reports[0];
    assert_eq!(
        report.source_name,
        "patterns/01-sharded-counter/naive/src/lib.rs"
    );
    assert_eq!(report.functions.len(), 2);

    let increment = &report.functions[0];
    assert_eq!(increment.storage_reads[0].render(), "Counter");
    // An empty segment list is core's dynamic-key marker.
    assert_eq!(report.functions[1].storage_writes[0].render(), "(dynamic)");

    // `function` is null for the global-static-write finding and must parse.
    let global = report
        .detectors
        .iter()
        .find(|d| d.detector == "global-static-write")
        .unwrap();
    assert_eq!(global.function, None);
    assert_eq!(global.key.as_deref(), Some("Counter"));
}

// ---------------------------------------------------------------------------
// Whole-corpus run (`harness all`): discovery, batch run, table rendering.
// ---------------------------------------------------------------------------

/// A private temp dir, removed when the test finishes.
fn tmp_dir(tag: &str) -> PathBuf {
    let base =
        std::env::temp_dir().join(format!("slipstream-harness-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn mkdirs(base: &Path, paths: &[&str]) {
    for p in paths {
        std::fs::create_dir_all(base.join(p)).unwrap();
    }
}

#[test]
fn discover_patterns_finds_complete_and_incomplete() {
    let base = tmp_dir("discover");
    mkdirs(
        &base,
        &[
            "01-alpha/naive",
            "01-alpha/optimized",
            "02-beta/naive", // no optimized side
            "03-gamma",      // no sides at all
            "src",           // non-pattern dir, ignored
        ],
    );
    std::fs::write(base.join("README.md"), "").unwrap(); // non-pattern file, ignored

    let patterns = discover_patterns(&base);
    let names: Vec<&str> = patterns.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["01-alpha", "02-beta", "03-gamma"]);

    assert!(patterns[0].is_complete(), "01 has both sides");
    assert!(patterns[0].naive.is_some() && patterns[0].optimized.is_some());

    assert!(!patterns[1].is_complete());
    assert!(patterns[1].naive.is_some());
    assert!(patterns[1].optimized.is_none());

    assert!(!patterns[2].is_complete());
    assert!(patterns[2].naive.is_none());
    assert!(patterns[2].optimized.is_none());

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn discover_patterns_ignores_non_pattern_names() {
    let base = tmp_dir("nonpatterns");
    mkdirs(
        &base,
        &["05x/naive", "x/naive", "01-x/naive", "01-x/optimized"],
    );
    let patterns = discover_patterns(&base);
    let names: Vec<&str> = patterns.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["01-x"], "only NN-<name> dirs are patterns");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn run_all_reports_missing_sides_without_aborting() {
    let base = tmp_dir("runall");
    mkdirs(
        &base,
        &[
            "01-good/naive",
            "01-good/optimized",
            "02-no-opt/naive",
            "03-no-naive/optimized",
        ],
    );
    let patterns = discover_patterns(&base);
    let runner = MockRunner::with_diff(SAMPLE_DIFF);
    let results = run_all(&runner, &patterns);
    assert_eq!(results.len(), 3);

    assert_eq!(results[0].pattern, "01-good");
    assert!(results[0].delta.is_some());
    assert!(results[0].missing_sides.is_empty());
    assert_eq!(results[0].verdict(), "IMPROVEMENT");

    assert_eq!(results[1].pattern, "02-no-opt");
    assert!(results[1].delta.is_none());
    assert_eq!(results[1].missing_sides, vec!["optimized"]);
    assert_eq!(results[1].verdict(), "missing optimized");

    assert_eq!(results[2].pattern, "03-no-naive");
    assert!(results[2].delta.is_none());
    assert_eq!(results[2].missing_sides, vec!["naive"]);

    let _ = std::fs::remove_dir_all(&base);
}

/// A runner that fails for any pair whose naive path contains "bad".
struct FlakyRunner;

impl CoreRunner for FlakyRunner {
    fn diff(&self, left: &Path, _right: &Path) -> Result<String, HarnessError> {
        if left.to_string_lossy().contains("bad") {
            Err(HarnessError::Spawn("boom".into()))
        } else {
            Ok(SAMPLE_DIFF.to_string())
        }
    }

    fn scan(&self, _path: &Path) -> Result<String, HarnessError> {
        Ok(String::new())
    }
}

#[test]
fn run_all_continues_past_a_failed_pair() {
    let base = tmp_dir("flaky");
    mkdirs(
        &base,
        &[
            "01-good/naive",
            "01-good/optimized",
            "02-bad/naive",
            "02-bad/optimized",
        ],
    );
    let patterns = discover_patterns(&base);
    let results = run_all(&FlakyRunner, &patterns);
    assert_eq!(results.len(), 2);

    assert!(results[0].delta.is_some(), "good pair still analyzed");
    assert!(results[0].error.is_none());

    assert!(results[1].delta.is_none());
    assert!(results[1].error.is_some(), "failure is recorded, not fatal");
    assert_eq!(
        results[1].verdict(),
        "error: failed to launch slipstream: boom"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn render_table_has_one_row_per_pattern_with_verdicts() {
    let results = vec![
        PatternResult {
            pattern: "01-alpha".into(),
            delta: Some(ContentionDelta {
                storage_writes_delta: -2,
                storage_reads_delta: 3,
                detector_findings_delta: -1,
            }),
            missing_sides: Vec::new(),
            error: None,
        },
        PatternResult {
            pattern: "02-beta".into(),
            delta: None,
            missing_sides: vec!["optimized"],
            error: None,
        },
        PatternResult {
            pattern: "03-gamma".into(),
            delta: None,
            missing_sides: Vec::new(),
            error: Some("slipstream exited with Some(1)".into()),
        },
    ];
    let table = render_table(&results);
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 5, "header + divider + one row per pattern");

    assert!(lines[0].contains("pattern"));
    assert!(lines[0].contains("writes"));
    assert!(lines[0].contains("reads"));
    assert!(lines[0].contains("findings"));
    assert!(lines[0].contains("verdict"));

    assert!(lines[2].contains("01-alpha"));
    assert!(lines[2].contains("-2"));
    assert!(lines[2].contains("IMPROVEMENT"));

    assert!(lines[3].contains("02-beta"));
    assert!(lines[3].contains("missing optimized"));

    assert!(lines[4].contains("03-gamma"));
    assert!(lines[4].contains("error: slipstream exited with Some(1)"));
}

#[test]
fn pattern_is_complete_only_with_both_sides() {
    let base = tmp_dir("complete");
    mkdirs(&base, &["01-x/naive", "01-x/optimized"]);
    let patterns = discover_patterns(&base);
    let p: &Pattern = &patterns[0];
    assert!(p.is_complete());
    assert_eq!(p.naive, Some(base.join("01-x/naive")));
    assert_eq!(p.optimized, Some(base.join("01-x/optimized")));
    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// `harness bench`: measured deltas written into BENCH.md with provenance.
// ---------------------------------------------------------------------------

fn sample_provenance() -> BenchProvenance {
    BenchProvenance {
        os: "linux".into(),
        arch: "x86_64".into(),
        rustc: "rustc 1.85.0".into(),
        slipstream_version: "slipstream 0.1.0".into(),
        contracts_sha: "contracts-sha".into(),
        core_sha: "core-sha".into(),
        run_time: "12345".into(),
        command: "slipstream-harness bench".into(),
    }
}

#[test]
fn bench_block_contains_deltas_and_provenance() {
    let delta = ContentionDelta {
        storage_writes_delta: -1,
        storage_reads_delta: 8,
        detector_findings_delta: 0,
    };
    let block = render_bench_block("01-alpha", &delta, &sample_provenance());

    assert!(block.contains(crate::BENCH_BEGIN));
    assert!(block.contains(crate::BENCH_END));
    assert!(block.contains("01-alpha"));
    assert!(block.contains("| storage writes | -1 |"));
    assert!(block.contains("| storage reads | +8 |"));
    assert!(block.contains("| detector findings | 0 |"));
    assert!(block.contains("contracts@contracts-sha"));
    assert!(block.contains("core@core-sha"));
    assert!(block.contains("slipstream 0.1.0"));
    assert!(
        block.ends_with(crate::BENCH_END),
        "block ends at the END marker"
    );
}

#[test]
fn signed_delta_formats_plus_for_positives() {
    assert_eq!(signed_delta(-3), "-3");
    assert_eq!(signed_delta(0), "0");
    assert_eq!(signed_delta(7), "+7");
}

#[test]
fn update_bench_md_appends_when_missing() {
    let base = tmp_dir("bench-append");
    let path = base.join("BENCH.md");
    std::fs::write(&path, "# Pattern\n\nbody\n").unwrap();

    let block = render_bench_block(
        "01-alpha",
        &ContentionDelta {
            storage_writes_delta: -1,
            storage_reads_delta: 2,
            detector_findings_delta: -1,
        },
        &sample_provenance(),
    );
    assert!(update_bench_md(&path, &block).unwrap());

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(
        content.starts_with("# Pattern\n\nbody\n"),
        "existing content preserved"
    );
    assert_eq!(content.matches(crate::BENCH_BEGIN).count(), 1);
    assert_eq!(content.matches(crate::BENCH_END).count(), 1);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn update_bench_md_replaces_idempotently() {
    let base = tmp_dir("bench-replace");
    let path = base.join("BENCH.md");
    std::fs::write(&path, "# Pattern\n\nbody\n").unwrap();

    let block_a = render_bench_block(
        "01-alpha",
        &ContentionDelta {
            storage_writes_delta: -1,
            storage_reads_delta: 2,
            detector_findings_delta: -1,
        },
        &sample_provenance(),
    );
    let block_b = render_bench_block(
        "01-alpha",
        &ContentionDelta {
            storage_writes_delta: -2,
            storage_reads_delta: 3,
            detector_findings_delta: 0,
        },
        &sample_provenance(),
    );

    assert!(
        update_bench_md(&path, &block_a).unwrap(),
        "first write changes the file"
    );
    assert!(
        !update_bench_md(&path, &block_a).unwrap(),
        "same block is a no-op"
    );
    assert!(
        update_bench_md(&path, &block_b).unwrap(),
        "new block replaces the old"
    );

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        content.matches(crate::BENCH_BEGIN).count(),
        1,
        "exactly one block"
    );
    assert!(content.contains("| storage writes | -2 |"));
    assert!(!content.contains("| storage writes | -1 |"));
    assert!(
        content.starts_with("# Pattern\n\nbody\n"),
        "surrounding text untouched"
    );
    let _ = std::fs::remove_dir_all(&base);
}
