//! CLI entry point for the Slipstream contract harness.
//!
//! Usage:
//! ```text
//! slipstream-harness <pattern-dir>
//! slipstream-harness all [patterns-dir]
//! ```
//!
//! `slipstream-harness <pattern-dir>` analyzes a single pattern pair. The
//! harness expects `<pattern-dir>/naive` and `<pattern-dir>/optimized`, runs
//! `slipstream diff` over them via [`SubprocessRunner`], and prints the
//! contention delta.
//!
//! `slipstream-harness all` discovers every `patterns/NN-*/` directory, runs
//! every complete naive/optimized pair, and prints a summary table. Patterns
//! missing a side are reported in the table, not fatal. The `slipstream`
//! binary must be on `PATH` (or pointed at by `SLIPSTREAM_BIN`); when it is
//! absent the command fails fast with an actionable message.
//!
//! [`SubprocessRunner`]: harness::SubprocessRunner

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use harness::{
    analyze_pair, discover_patterns, render_table, run_all, ContentionDelta, HarnessError,
    PatternResult, SubprocessRunner,
};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let first = match args.next() {
        Some(arg) => arg,
        None => {
            usage();
            return ExitCode::from(2);
        }
    };

    if first == "all" {
        let patterns_dir = args
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("patterns"));
        return run_all_cmd(&patterns_dir);
    }

    let pattern_dir = PathBuf::from(first);
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

fn usage() {
    eprintln!("usage: slipstream-harness <pattern-dir>");
    eprintln!("       slipstream-harness all [patterns-dir]");
    eprintln!("  <pattern-dir> expects <pattern-dir>/naive and <pattern-dir>/optimized");
}

fn run(pattern_dir: &Path) -> Result<ContentionDelta, HarnessError> {
    let naive = pattern_dir.join("naive");
    let optimized = pattern_dir.join("optimized");
    let runner = SubprocessRunner::from_env();
    analyze_pair(&runner, &naive, &optimized)
}

fn run_all_cmd(patterns_dir: &Path) -> ExitCode {
    let runner = SubprocessRunner::from_env();
    if !runner.available() {
        eprintln!(
            "error: the `slipstream` binary was not found on PATH (or at SLIPSTREAM_BIN={})",
            std::env::var("SLIPSTREAM_BIN").unwrap_or_else(|_| "<unset>".into())
        );
        eprintln!(
            "hint: build it from slipstream-core, e.g. `cargo build -p slipstream-cli` in \
             slipstream-core, then set SLIPSTREAM_BIN to the binary."
        );
        return ExitCode::from(2);
    }

    let patterns = discover_patterns(patterns_dir);
    if patterns.is_empty() {
        eprintln!(
            "error: no `NN-<name>` pattern directories found under {}",
            patterns_dir.display()
        );
        return ExitCode::from(2);
    }

    let results = run_all(&runner, &patterns);
    print!("{}", render_table(&results));
    summarize(&results);
    let incomplete = results
        .iter()
        .any(|r| !r.missing_sides.is_empty() || r.error.is_some());
    if incomplete {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn summarize(results: &[PatternResult]) {
    let (ok, missing, failed) = results
        .iter()
        .fold((0usize, 0usize, 0usize), |(o, m, f), r| {
            if r.delta.is_some() {
                (o + 1, m, f)
            } else if !r.missing_sides.is_empty() {
                (o, m + 1, f)
            } else {
                (o, m, f + 1)
            }
        });
    let improved = results
        .iter()
        .filter(|r| r.delta.as_ref().is_some_and(|d| d.is_improvement()))
        .count();
    if ok > 0 {
        eprintln!(
            "summary: {ok} analyzed ({improved} improvements), {missing} missing sides, {failed} failed"
        );
    } else {
        eprintln!("summary: nothing was analyzed");
    }
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
