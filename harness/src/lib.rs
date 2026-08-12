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

use std::path::Path;

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
