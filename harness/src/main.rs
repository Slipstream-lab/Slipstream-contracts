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
//! missing a side are reported in the table, not fatal.
//!
//! `slipstream-harness bench` does the same run and writes the measured deltas
//! plus a provenance snapshot into each pattern's `BENCH.md` (idempotently).
//! No numbers are fabricated: if the `slipstream` binary is absent the command
//! fails fast and the tables stay `TBD`.
//!
//! [`SubprocessRunner`]: harness::SubprocessRunner

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use harness::{
    analyze_pair, discover_patterns, render_bench_block, render_table, run_all, update_bench_md,
    BenchProvenance, ContentionDelta, HarnessError, PatternResult, SubprocessRunner,
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

    if first == "bench" {
        let patterns_dir = args
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("patterns"));
        return bench_cmd(&patterns_dir);
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
    eprintln!("       slipstream-harness bench [patterns-dir]");
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

fn bench_cmd(patterns_dir: &Path) -> ExitCode {
    let bin = std::env::var("SLIPSTREAM_BIN").unwrap_or_else(|_| "slipstream".to_string());
    let runner = SubprocessRunner::new(bin.clone());
    if !runner.available() {
        eprintln!(
            "error: the `slipstream` binary was not found on PATH (or at SLIPSTREAM_BIN={bin})"
        );
        eprintln!(
            "hint: build it from slipstream-core, then set SLIPSTREAM_BIN; tables stay TBD until then."
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
    let prov = collect_provenance(&bin, patterns_dir);

    let mut updated = 0usize;
    let mut unchanged = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    for (p, r) in patterns.iter().zip(&results) {
        let Some(delta) = &r.delta else {
            skipped += 1;
            continue;
        };
        let bench_md = p.dir.join("BENCH.md");
        let block = render_bench_block(&p.name, delta, &prov);
        match update_bench_md(&bench_md, &block) {
            Ok(true) => updated += 1,
            Ok(false) => unchanged += 1,
            Err(e) => {
                failed += 1;
                eprintln!("error: could not update {}: {e}", bench_md.display());
            }
        }
    }

    eprintln!(
        "bench: {updated} BENCH.md updated, {unchanged} already current, {skipped} skipped (incomplete), {failed} failed"
    );
    eprintln!(
        "provenance: contracts@{}, core@{}, slipstream={}, run={}",
        prov.contracts_sha, prov.core_sha, prov.slipstream_version, prov.run_time
    );
    if updated > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Run a command and capture its stdout on success.
fn sh(prog: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(prog)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn unix_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn collect_provenance(bin: &str, patterns_dir: &Path) -> BenchProvenance {
    let contracts_sha = sh(
        "git",
        &["-C", &patterns_dir.to_string_lossy(), "rev-parse", "HEAD"],
    )
    .unwrap_or_else(|| "unknown".into());
    let core_dir = std::env::var("SLIPSTREAM_CORE_DIR").unwrap_or_default();
    let core_sha = if core_dir.is_empty() {
        "unknown (set SLIPSTREAM_CORE_DIR)".into()
    } else {
        sh("git", &["-C", &core_dir, "rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into())
    };
    BenchProvenance {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        rustc: sh("rustc", &["--version"]).unwrap_or_else(|| "unknown".into()),
        slipstream_version: sh(bin, &["--version"]).unwrap_or_else(|| "unknown".into()),
        contracts_sha,
        core_sha,
        run_time: unix_ts(),
        command: std::env::args().collect::<Vec<_>>().join(" "),
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
