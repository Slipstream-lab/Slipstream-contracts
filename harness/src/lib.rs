//! Harness for driving [`slipstream-core`] over the reference pattern corpus.
//!
//! For each pattern the harness runs `slipstream diff <naive> <optimized>
//! --json` (and optionally `slipstream scan --json`) and reports the
//! *contention delta*: how much the optimized variant reduces storage writes,
//! detector findings, etc. relative to the naive one.
//!
//! `slipstream-core` is a separate binary. To keep this crate buildable and
//! testable in isolation, all interaction goes through the [`CoreRunner`]
//! trait: [`SubprocessRunner`] shells out to a real `slipstream` binary, while
//! [`MockRunner`] returns canned JSON for tests. The delta computation in
//! [`contention_delta`] is pure and unit-tested against the mock.
//!
//! [`slipstream-core`]: https://github.com/Slipstream-lab/Slipstream-core

use std::path::{Path, PathBuf};

pub mod model;

use model::DiffReport;

/// Errors the harness can surface.
#[derive(Debug)]
pub enum HarnessError {
    /// The core binary could not be launched.
    Spawn(String),
    /// The core binary ran but exited non-zero.
    Command { code: Option<i32>, stderr: String },
    /// The core binary's output was not valid JSON in the expected shape.
    Parse(String),
}
impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HarnessError::Spawn(msg) => write!(f, "failed to launch slipstream: {msg}"),
            HarnessError::Command { code, stderr } => {
                write!(f, "slipstream exited with {code:?}: {stderr}")
            }
            HarnessError::Parse(msg) => write!(f, "failed to parse slipstream output: {msg}"),
        }
    }
}

impl std::error::Error for HarnessError {}

/// Abstraction over "run slipstream-core and give me its JSON".
///
/// Implementors return the raw stdout of the corresponding `slipstream`
/// subcommand; parsing is done by the harness so the same parsing path is
/// exercised by both the real and mock runners.
pub trait CoreRunner {
    /// Run `slipstream diff <left> <right> --json` and return raw stdout.
    fn diff(&self, left: &Path, right: &Path) -> Result<String, HarnessError>;

    /// Run `slipstream scan <path> --json` and return raw stdout.
    fn scan(&self, path: &Path) -> Result<String, HarnessError>;
}

/// The contention improvement of an optimized variant over its naive sibling.
///
/// Deltas are computed as `optimized - naive`, so *negative* numbers mean the
/// optimized variant does fewer of the thing (which is the goal for writes and
/// detector findings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentionDelta {
    /// Change in number of storage writes across the whole contract.
    pub storage_writes_delta: i64,
    /// Change in number of storage reads across the whole contract.
    pub storage_reads_delta: i64,
    /// Change in number of detector findings (contention anti-patterns flagged).
    pub detector_findings_delta: i64,
}

impl ContentionDelta {
    /// True when the optimized variant strictly reduces contention signals
    /// (fewer writes AND no more detector findings than the naive variant).
    pub fn is_improvement(&self) -> bool {
        self.storage_writes_delta < 0 && self.detector_findings_delta <= 0
    }
}

/// Compute the contention delta from a parsed diff report.
///
/// This is intentionally a pure function of the report so it can be unit-tested
/// without any subprocess. The summary the core emits already contains the
/// aggregate deltas; we surface them in a typed, harness-facing struct.
pub fn contention_delta(report: &DiffReport) -> ContentionDelta {
    ContentionDelta {
        storage_writes_delta: report.summary.storage_writes_delta,
        storage_reads_delta: report.summary.storage_reads_delta,
        detector_findings_delta: report.summary.detector_findings_delta,
    }
}

/// Run a diff through the given runner and compute the contention delta.
pub fn analyze_pair(
    runner: &dyn CoreRunner,
    naive: &Path,
    optimized: &Path,
) -> Result<ContentionDelta, HarnessError> {
    let raw = runner.diff(naive, optimized)?;
    let report: DiffReport =
        serde_json::from_str(&raw).map_err(|e| HarnessError::Parse(e.to_string()))?;
    Ok(contention_delta(&report))
}

// ---------------------------------------------------------------------------
// Whole-corpus runs (`harness all`)
// ---------------------------------------------------------------------------

/// A pattern discovered under `patterns/`, with whatever sides are present.
///
/// A pattern directory is any entry named `NN-<name>` (two digits, a dash,
/// then a name). A missing `naive/` or `optimized/` side is recorded, not
/// fatal — the corpus is allowed to hold in-progress patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// Full directory name, e.g. `01-sharded-counter`.
    pub name: String,
    /// Path to the pattern directory itself.
    pub dir: PathBuf,
    /// `dir/naive` when it exists.
    pub naive: Option<PathBuf>,
    /// `dir/optimized` when it exists.
    pub optimized: Option<PathBuf>,
}

impl Pattern {
    /// True when both sides exist and the pair can be analyzed.
    pub fn is_complete(&self) -> bool {
        self.naive.is_some() && self.optimized.is_some()
    }
}

/// Discover every `patterns/NN-*/` directory below `patterns_dir`.
///
/// Entries that do not look like `NN-<name>` are ignored so the directory may
/// also hold non-pattern files or directories.
pub fn discover_patterns(patterns_dir: &Path) -> Vec<Pattern> {
    let mut patterns = Vec::new();
    let Ok(entries) = std::fs::read_dir(patterns_dir) else {
        return patterns;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };
        if !is_pattern_name(&name) {
            continue;
        }
        let naive = path.join("naive");
        let optimized = path.join("optimized");
        patterns.push(Pattern {
            name,
            dir: path,
            naive: naive.is_dir().then_some(naive),
            optimized: optimized.is_dir().then_some(optimized),
        });
    }
    patterns.sort_by(|a, b| a.name.cmp(&b.name));
    patterns
}

/// A `NN-<name>` pattern directory name: two ASCII digits then a dash.
fn is_pattern_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_digit() && bytes[1].is_ascii_digit() && bytes[2] == b'-'
}

/// The outcome of analyzing one pattern in a batch run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternResult {
    /// Pattern name (same as [`Pattern::name`]).
    pub pattern: String,
    /// Computed delta when the pair was complete and analyzed.
    pub delta: Option<ContentionDelta>,
    /// Which sides were missing (`"naive"` / `"optimized"`).
    pub missing_sides: Vec<&'static str>,
    /// Analysis error for this pair, if any (e.g. core exited non-zero).
    pub error: Option<String>,
}

impl PatternResult {
    /// Human verdict for the table: an improvement summary, a missing-side
    /// note, or the error.
    pub fn verdict(&self) -> String {
        if !self.missing_sides.is_empty() {
            return format!("missing {}", self.missing_sides.join(" and "));
        }
        if let Some(err) = &self.error {
            return format!("error: {err}");
        }
        match &self.delta {
            Some(d) if d.is_improvement() => "IMPROVEMENT".to_string(),
            Some(_) => "NO IMPROVEMENT".to_string(),
            None => "unknown".to_string(),
        }
    }
}

/// Run every complete pattern pair, collecting results.
///
/// Missing sides never abort the batch: they are recorded in the result and
/// the run continues. A pattern whose pair is complete but fails to analyze
/// (e.g. core exits non-zero) is recorded as an error and the run continues.
pub fn run_all(runner: &dyn CoreRunner, patterns: &[Pattern]) -> Vec<PatternResult> {
    patterns
        .iter()
        .map(|p| {
            let (Some(naive), Some(optimized)) = (&p.naive, &p.optimized) else {
                let mut missing = Vec::new();
                if p.naive.is_none() {
                    missing.push("naive");
                }
                if p.optimized.is_none() {
                    missing.push("optimized");
                }
                return PatternResult {
                    pattern: p.name.clone(),
                    delta: None,
                    missing_sides: missing,
                    error: None,
                };
            };
            match analyze_pair(runner, naive, optimized) {
                Ok(delta) => PatternResult {
                    pattern: p.name.clone(),
                    delta: Some(delta),
                    missing_sides: Vec::new(),
                    error: None,
                },
                Err(e) => PatternResult {
                    pattern: p.name.clone(),
                    delta: None,
                    missing_sides: Vec::new(),
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}

/// Render a one-row-per-pattern summary table.
///
/// Columns: pattern, storage-writes delta, storage-reads delta, detector
/// findings delta, verdict. Incomplete or failed patterns show `-` for the
/// deltas and their reason in the verdict column.
pub fn render_table(results: &[PatternResult]) -> String {
    const HEADERS: [&str; 5] = ["pattern", "writes", "reads", "findings", "verdict"];

    fn cell_delta(d: &Option<ContentionDelta>, pick: fn(&ContentionDelta) -> i64) -> String {
        d.as_ref()
            .map(pick)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    }

    let rows: Vec<[String; 5]> = results
        .iter()
        .map(|r| {
            [
                r.pattern.clone(),
                cell_delta(&r.delta, |d| d.storage_writes_delta),
                cell_delta(&r.delta, |d| d.storage_reads_delta),
                cell_delta(&r.delta, |d| d.detector_findings_delta),
                r.verdict(),
            ]
        })
        .collect();

    let mut widths = HEADERS.map(str::len);
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    let mut out = String::new();
    let header_line = HEADERS
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if i == 0 {
                format!("{h:<width$}", width = widths[i])
            } else {
                format!("{h:>width$}", width = widths[i])
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    out.push_str(&header_line);
    out.push('\n');
    out.push_str(&"-".repeat(header_line.len()));
    out.push('\n');
    for row in &rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                if i == 0 {
                    format!("{cell:<width$}", width = widths[i])
                } else {
                    format!("{cell:>width$}", width = widths[i])
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Real runner: shells out to the `slipstream` binary.
// ---------------------------------------------------------------------------

/// [`CoreRunner`] that invokes a real `slipstream` binary as a subprocess.
///
/// The binary path is taken from the `SLIPSTREAM_BIN` environment variable,
/// defaulting to `slipstream` (i.e. resolved on `PATH`). Running the harness
/// against real core therefore requires `slipstream-core` to be installed and
/// on `PATH`, or `SLIPSTREAM_BIN` to point at it.
pub struct SubprocessRunner {
    bin: String,
}

impl SubprocessRunner {
    /// Build a runner, reading `SLIPSTREAM_BIN` (default `slipstream`).
    pub fn from_env() -> Self {
        let bin = std::env::var("SLIPSTREAM_BIN").unwrap_or_else(|_| "slipstream".to_string());
        SubprocessRunner { bin }
    }

    /// Build a runner pointing at an explicit binary path.
    pub fn new(bin: impl Into<String>) -> Self {
        SubprocessRunner { bin: bin.into() }
    }

    /// True when the configured binary can be launched at all.
    ///
    /// Used to preflight a batch run so a missing `slipstream` is reported
    /// once, up front, instead of as a failure per pattern.
    pub fn available(&self) -> bool {
        std::process::Command::new(&self.bin)
            .arg("--version")
            .output()
            .is_ok()
    }

    fn run(&self, args: &[&str]) -> Result<String, HarnessError> {
        let output = std::process::Command::new(&self.bin)
            .args(args)
            .output()
            .map_err(|e| HarnessError::Spawn(e.to_string()))?;
        if !output.status.success() {
            return Err(HarnessError::Command {
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        String::from_utf8(output.stdout).map_err(|e| HarnessError::Parse(e.to_string()))
    }
}

impl CoreRunner for SubprocessRunner {
    fn diff(&self, left: &Path, right: &Path) -> Result<String, HarnessError> {
        self.run(&[
            "diff",
            &left.to_string_lossy(),
            &right.to_string_lossy(),
            "--json",
        ])
    }

    fn scan(&self, path: &Path) -> Result<String, HarnessError> {
        self.run(&["scan", &path.to_string_lossy(), "--json"])
    }
}

// ---------------------------------------------------------------------------
// Mock runner: canned JSON, for tests and offline development.
// ---------------------------------------------------------------------------

/// [`CoreRunner`] that returns pre-set JSON strings instead of spawning
/// anything. Used by the harness's own tests and handy for offline demos.
#[derive(Default)]
pub struct MockRunner {
    /// Raw JSON returned by [`CoreRunner::diff`].
    pub diff_json: String,
    /// Raw JSON returned by [`CoreRunner::scan`].
    pub scan_json: String,
}

impl MockRunner {
    /// Build a mock that returns `diff_json` from `diff`.
    pub fn with_diff(diff_json: impl Into<String>) -> Self {
        MockRunner {
            diff_json: diff_json.into(),
            scan_json: String::new(),
        }
    }
}

impl CoreRunner for MockRunner {
    fn diff(&self, _left: &Path, _right: &Path) -> Result<String, HarnessError> {
        Ok(self.diff_json.clone())
    }

    fn scan(&self, _path: &Path) -> Result<String, HarnessError> {
        Ok(self.scan_json.clone())
    }
}

#[cfg(test)]
mod test;
