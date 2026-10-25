//! examples/dataset_prepare.rs
//!
//! cargo run --release --example dataset_prepare -- <file> [file...]
//!
//! S2ELab only: extracts, translates, and writes every usable pattern's
//! own given match/non-match examples, each checked against the real
//! translated `Regex`; a claim that disagrees is dropped and counted.
//! A pattern needs at least one match and one non-match left after that
//! to be usable; below that, it is counted, not dropped. Every usable
//! pattern is then truncated to its own balanced match/non-match count
//! -- patterns are not truncated against each other, so a pattern with
//! more given examples keeps all of them; see `src/data/mod.rs`'s
//! module doc for why.
//!
//! Appends one row per run to `reports/data/dataset_prepare.csv`.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use regex_engine::data::run_pipeline;

fn usage() -> &'static str {
    "usage: cargo run --release --example dataset_prepare -- <file> [file...]"
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut files: Vec<PathBuf> = Vec::new();
    for a in &args[1..] {
        if a == "--" {
            // Cargo has already stripped its own separator; a literal
            // `--` reaching here is tolerated and ignored.
        } else if a.starts_with("--") {
            eprintln!("unknown argument {a:?}");
            eprintln!("{}", usage());
            std::process::exit(2);
        } else {
            files.push(PathBuf::from(a.as_str()));
        }
    }

    if files.is_empty() {
        eprintln!("{}", usage());
        std::process::exit(2);
    }

    let data_root = Path::new("data/processed");

    let report = run_pipeline(&files, data_root).unwrap_or_else(|e| {
        eprintln!("pipeline failed: {e}");
        std::process::exit(1);
    });

    print_report(&report);
    append_report_csv(&files, &report);
}

/// Appends one row per run to `reports/data/dataset_prepare.csv`.
/// Writes the header only when the file does not already exist, so
/// repeated runs accumulate rows instead of repeating the header
/// partway through the file.
fn append_report_csv(files: &[PathBuf], report: &regex_engine::data::PipelineReport) {
    let dir = "reports/data";
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not create {dir}: {e}");
        return;
    }
    let path = format!("{dir}/dataset_prepare.csv");
    let write_header = !Path::new(&path).exists();
    let mut f = match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not open {path}: {e}");
            return;
        }
    };
    if write_header {
        if let Err(e) = writeln!(
            f,
            "files,patterns_extracted,patterns_duplicate,patterns_unusable,patterns_accepted,\
             patterns_explicit_full_match,patterns_unanchored_ambiguous,patterns_usable,\
             inputs_usable,inputs_label_mismatch,inputs_dropped_for_pattern_balance"
        ) {
            eprintln!("could not write header to {path}: {e}");
            return;
        }
    }
    let files_joined = files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(";");
    if let Err(e) = writeln!(
        f,
        "\"{}\",{},{},{},{},{},{},{},{},{},{}",
        files_joined,
        report.patterns_extracted,
        report.patterns_duplicate,
        report.patterns_unusable,
        report.patterns_accepted,
        report.patterns_explicit_full_match,
        report.patterns_unanchored_ambiguous,
        report.patterns_usable,
        report.inputs_usable,
        report.inputs_label_mismatch,
        report.inputs_dropped_for_pattern_balance,
    ) {
        eprintln!("could not append to {path}: {e}");
        return;
    }
    eprintln!("appended to {path}");
}

fn print_report(report: &regex_engine::data::PipelineReport) {
    let rule = "─".repeat(72);

    println!("S2ELab");
    println!("{rule}");

    println!();
    println!("Extraction");
    println!("  extracted           {:>7}", report.patterns_extracted);
    if report.patterns_duplicate > 0 {
        println!(
            "  duplicate pattern   {:>7}   (byte-identical to an earlier pattern; skipped)",
            report.patterns_duplicate,
        );
    }
    println!("  unusable (frontend) {:>7}", report.patterns_unusable);
    println!(
        "  accepted (frontend) {:>7}   [{} direct, {} full-match]",
        report.patterns_accepted,
        report.patterns_unanchored_ambiguous,
        report.patterns_explicit_full_match,
    );

    println!();
    println!("Usable corpus");
    if report.inputs_label_mismatch > 0 {
        println!(
            "  label mismatch   {:>7}   (claim disagreed with the real Regex; dropped)",
            report.inputs_label_mismatch,
        );
    }
    println!(
        "  patterns usable  {:>7}   of {} accepted  (>= 1 match + >= 1 non-match each)",
        report.patterns_usable, report.patterns_accepted,
    );
    if report.inputs_dropped_for_pattern_balance > 0 {
        println!(
            "  pattern balance  {:>7}   (per-pattern: longer side truncated to the shorter)",
            report.inputs_dropped_for_pattern_balance,
        );
    }
    println!("  inputs written   {:>7}", report.inputs_usable);

    println!();
    println!("  wrote  data/processed/s2-elab/prepared.jsonl");
    println!(
        "  {} patterns, {} inputs total",
        report.patterns_usable, report.inputs_usable,
    );
}
