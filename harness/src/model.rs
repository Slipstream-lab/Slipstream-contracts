//! Typed model of the `slipstream-core` JSON contract.
//!
//! These structs mirror the JSON emitted by `slipstream diff --json` and
//! `slipstream scan --json`. They use `#[serde(default)]` liberally so the
//! harness keeps working if core adds fields or omits empty ones.

use serde::Deserialize;

// ---------------------------------------------------------------------------
// `slipstream diff <left> <right> --json`
// ---------------------------------------------------------------------------

/// Top-level output of `slipstream diff --json`.
#[derive(Debug, Clone, Deserialize)]
pub struct DiffReport {
    /// Analysis of the left (naive) side.
    pub left: SideReport,
    /// Analysis of the right (optimized) side.
    pub right: SideReport,
    /// Per-function read/write deltas (`right - left`).
    #[serde(default)]
    pub per_function_deltas: Vec<PerFunctionDelta>,
    /// Aggregate deltas.
    pub summary: DiffSummary,
}

/// One side of a diff (either the naive or the optimized contract).
#[derive(Debug, Clone, Deserialize)]
pub struct SideReport {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub files: u64,
    #[serde(default)]
    pub functions: u64,
    #[serde(default)]
    pub storage_reads: u64,
    #[serde(default)]
    pub storage_writes: u64,
    #[serde(default)]
    pub detector_findings: u64,
    /// Per-detector counts, keyed by detector name.
    #[serde(default)]
    pub detectors: std::collections::BTreeMap<String, u64>,
}

/// Read/write deltas for a single function present on both sides.
#[derive(Debug, Clone, Deserialize)]
pub struct PerFunctionDelta {
    pub function: String,
    pub reads_delta: i64,
    pub writes_delta: i64,
}

/// Aggregate `right - left` deltas.
#[derive(Debug, Clone, Deserialize)]
pub struct DiffSummary {
    #[serde(default)]
    pub detector_findings_delta: i64,
    #[serde(default)]
    pub storage_reads_delta: i64,
    #[serde(default)]
    pub storage_writes_delta: i64,
}

// ---------------------------------------------------------------------------
// `slipstream scan <path> --json`  (an array of these)
// ---------------------------------------------------------------------------

/// One entry in the array emitted by `slipstream scan --json`.
///
/// Field names mirror `slipstream-core`'s `AnalysisReport` exactly (note
/// `source_name`, not `source`).
#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisReport {
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub functions: Vec<FunctionReport>,
    #[serde(default)]
    pub detectors: Vec<DetectorFinding>,
}

/// Per-function storage footprint from a scan.
///
/// `storage_reads` / `storage_writes` are `slipstream-core` `StaticKey` values
/// (segment lists), not plain strings.
#[derive(Debug, Clone, Deserialize)]
pub struct FunctionReport {
    pub function_name: String,
    #[serde(default)]
    pub storage_reads: Vec<StaticKey>,
    #[serde(default)]
    pub storage_writes: Vec<StaticKey>,
}

/// A `slipstream-core` static storage key: a list of resolved segments. An
/// empty segment list is core's `(dynamic)` marker.
#[derive(Debug, Clone, Deserialize)]
pub struct StaticKey {
    #[serde(default)]
    pub segments: Vec<String>,
}

impl StaticKey {
    /// Renders the key the way core's text output does: dotted segments, or
    /// `(dynamic)` when unresolved.
    pub fn render(&self) -> String {
        if self.segments.is_empty() {
            "(dynamic)".to_string()
        } else {
            self.segments.join(".")
        }
    }
}

/// A single detector finding from a scan. `function` and `key` are optional in
/// core's output (e.g. `global-static-write` reports no single function).
#[derive(Debug, Clone, Deserialize)]
pub struct DetectorFinding {
    pub detector: String,
    #[serde(default)]
    pub function: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub message: String,
}
