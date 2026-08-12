use std::path::Path;

use crate::{analyze_pair, contention_delta, model::DiffReport, MockRunner};

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
