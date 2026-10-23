//! examples/dataset_prepare.rs
//!
//! cargo run --release --example dataset_prepare -- \
//!     <source> <file...> [--verbose]
//!
//! Extract patterns, skip byte-identical duplicates (`rules_duplicate_pattern`)
//! before translation, then keep only patterns with at least one verified
//! match and at least one verified non-match. Below that: counted, not
//! dropped.
//!
//! No input is generated, derived, or assembled -- every candidate comes
//! from the source's own given examples (today: S2ELab's `matches`/
//! `non_matches`), and every one of them is used, not a capped subset.
//! Suricata, SpamAssassin, RegexLib, and Innovatorved have no given-input
//! extraction wired in yet, so running this against them extracts and
//! translates their rules but produces zero usable patterns -- that is
//! the correct, expected result, not a bug.
//!
//! `--verbose`: list excluded patterns with raw text.
//! Appends one row per run to `reports/data/dataset_prepare.csv`.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use regex_engine::data::run_pipeline;
use regex_engine::data::types::SourceKind;

fn usage() -> &'static str {
    "usage: cargo run --release --example dataset_prepare -- \
     <suricata|spamassassin|regexlib|s2-elab|innovatorved> <file> [file...] [--verbose]"
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let Some(source_arg) = args.get(1).map(String::as_str) else {
        eprintln!("{}", usage());
        std::process::exit(2);
    };
    let source = match source_arg {
        "suricata" | "snort" => SourceKind::Suricata,
        "spamassassin" => SourceKind::SpamAssassin,
        "regexlib" => SourceKind::RegexLib,
        "s2-elab" | "s2elab" => SourceKind::S2ELab,
        "innovatorved" => SourceKind::Innovatorved,
        other => {
            eprintln!(
                "unknown source {other:?} (expected suricata, spamassassin, regexlib, s2-elab, or innovatorved)"
            );
            std::process::exit(2);
        }
    };

    let mut files: Vec<PathBuf> = Vec::new();
    let mut verbose = false;
    for a in &args[2..] {
        if a == "--verbose" {
            verbose = true;
        } else if a == "--" {
            // Cargo has already stripped its own separator; a literal
            // `--` reaching here is tolerated and ignored.
        } else if a.starts_with("--") {
            eprintln!("unknown argument {a:?}");
            std::process::exit(2);
        } else {
            files.push(PathBuf::from(a.as_str()));
        }
    }

    if files.is_empty() {
        eprintln!("no input files given");
        std::process::exit(2);
    }

    let data_root = Path::new("data/processed");

    let report = run_pipeline(source, &files, data_root).unwrap_or_else(|e| {
        eprintln!("pipeline failed: {e}");
        std::process::exit(1);
    });

    print_report(&report, source, verbose);
    append_report_csv(source, &files, &report);
}

/// Appends one row per run to `reports/data/dataset_prepare.csv`.
/// Writes the header only when the file does not already exist, so
/// repeated runs accumulate rows instead of repeating the header
/// partway through the file.
fn append_report_csv(
    source: SourceKind,
    files: &[PathBuf],
    report: &regex_engine::data::PipelineReport,
) {
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
            "source,files,rules_extracted,rules_duplicate_pattern,patterns_unusable,\
             patterns_accepted,patterns_unanchored_ambiguous,patterns_explicit_full_match,\
             inputs_generated,inputs_verified,inputs_rejected,inputs_skipped,\
             patterns_unusable_at_generation,patterns_usable,inputs_usable,\
             inputs_usable_from_source,inputs_usable_synthetic,inputs_dropped_with_pattern,\
             usable_explicit_full_match,usable_unanchored_ambiguous"
        ) {
            eprintln!("could not write header to {path}: {e}");
            return;
        }
    }
    let files_joined = files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(";");
    if let Err(e) = writeln!(
        f,
        "{:?},\"{}\",{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        source,
        files_joined,
        report.rules_extracted,
        report.rules_duplicate_pattern,
        report.patterns_unusable,
        report.patterns_accepted,
        report.patterns_unanchored_ambiguous,
        report.patterns_explicit_full_match,
        report.inputs_generated,
        report.inputs_verified,
        report.inputs_rejected,
        report.inputs_skipped(),
        report.patterns_unusable_at_generation,
        report.patterns_usable,
        report.inputs_usable,
        report.inputs_usable_from_source,
        report.inputs_usable_synthetic(),
        report.inputs_dropped_with_pattern(),
        report.usable_explicit_full_match,
        report.usable_unanchored_ambiguous,
    ) {
        eprintln!("could not append to {path}: {e}");
        return;
    }
    eprintln!("appended to {path}");
}

fn print_report(report: &regex_engine::data::PipelineReport, source: SourceKind, verbose: bool) {
    let rule = "─".repeat(72);

    println!("{:?}  --  using every given match/non-match example", source);
    println!("{rule}");

    println!();
    println!("Extraction");
    println!("  extracted           {:>7}", report.rules_extracted);
    if report.rules_duplicate_pattern > 0 {
        println!(
            "  duplicate pattern   {:>7}   (byte-identical to an earlier rule; skipped, not re-counted)",
            report.rules_duplicate_pattern,
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
    println!("Given examples");
    println!(
        "  candidates considered {:>7}   (every given match/non-match of every accepted pattern)",
        report.inputs_generated
    );
    println!(
        "  candidates verified   {:>7}   (examined and passed)",
        report.inputs_verified
    );
    println!(
        "  candidates rejected   {:>7}   (examined and failed the real parser)",
        report.inputs_rejected
    );

    println!();
    println!("Usable corpus");
    println!(
        "  patterns usable         {:>7}   of {} accepted  (>= 1 match + >= 1 non-match each)",
        report.patterns_usable, report.patterns_accepted,
    );
    if report.patterns_unusable_at_generation > 0 {
        println!(
            "  patterns unusable       {:>7}   (no verified match, no verified non-match, or no given examples at all)",
            report.patterns_unusable_at_generation,
        );
    }
    println!("  inputs written          {:>7}", report.inputs_usable);
    let dropped = report.inputs_dropped_with_pattern();
    if dropped > 0 {
        println!(
            "  dropped with pattern    {:>7}   (verified, but their pattern ended up unusable)",
            dropped,
        );
    }

    println!();
    println!("  wrote  data/processed/{}/prepared.jsonl", source.dir_name());
    println!(
        "  {} patterns, {} inputs total",
        report.patterns_usable, report.inputs_usable,
    );

    if verbose && !report.unusable_at_generation_patterns.is_empty() {
        println!();
        println!(
            "Unusable patterns ({} total):",
            report.unusable_at_generation_patterns.len()
        );
        for pattern in &report.unusable_at_generation_patterns {
            let shown: String = pattern.chars().take(100).collect();
            println!("  {shown}");
        }
    }
}
