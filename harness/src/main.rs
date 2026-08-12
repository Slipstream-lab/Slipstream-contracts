//! CLI entry point for the Slipstream contract harness.
//!
//! Usage:
//! ```text
//! slipstream-harness <pattern-dir>
//! slipstream-harness patterns/01-sharded-counter
//! ```
//!
//! The harness expects `<pattern-dir>/naive` and `<pattern-dir>/optimized`, runs
//! `slipstream diff` over them via [`SubprocessRunner`], and prints the
//! contention delta. The `slipstream` binary must be on `PATH` (or pointed at by
//! `SLIPSTREAM_BIN`); this repo does not vendor `slipstream-core`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use harness::{analyze_pair, ContentionDelta, HarnessError, SubprocessRunner};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let pattern_dir = match args.next() {
        Some(dir) => PathBuf::from(dir),
        None => {
            eprintln!("usage: slipstream-harness <pattern-dir>");
            eprintln!("  expects <pattern-dir>/naive and <pattern-dir>/optimized");
            return ExitCode::from(2);
        }
    };

    match run(&pattern_dir) {
        Ok(delta) => {
            report(&pattern_dir, &delta);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            if let HarnessError::Spawn(_) = e {
                eprintln!(
                    "hint: the `slipstream` binary must be on PATH or set via SLIPSTREAM_BIN; \
                     slipstream-core is not vendored in this repo."
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn run(pattern_dir: &Path) -> Result<ContentionDelta, HarnessError> {
    let naive = pattern_dir.join("naive");
    let optimized = pattern_dir.join("optimized");
    let runner = SubprocessRunner::from_env();
    analyze_pair(&runner, &naive, &optimized)
}

fn report(pattern_dir: &Path, delta: &ContentionDelta) {
    println!("pattern: {}", pattern_dir.display());
    println!("  storage_writes_delta:    {}", delta.storage_writes_delta);
    println!("  storage_reads_delta:     {}", delta.storage_reads_delta);
    println!(
        "  detector_findings_delta: {}",
        delta.detector_findings_delta
    );
    println!(
        "  verdict: {}",
        if delta.is_improvement() {
            "IMPROVEMENT (optimized reduces write contention)"
        } else {
            "NO IMPROVEMENT"
        }
    );
}
