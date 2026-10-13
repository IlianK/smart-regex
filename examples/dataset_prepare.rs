//! examples/dataset_prepare.rs
//!
//! cargo run --release --example dataset_prepare -- \
//!     <N> [--] <source> <file...> [--corpus-dir DIR] [--verbose]
//!
//! Extract patterns from the given raw file(s).
//! Generate or read off input per pattern, verify it, then balance it.
//! A pattern survives into `data/processed/<source>/prepared.jsonl` only
//! if it has at least N/2 verified matches and N/2 verified non-matches.
//! A surviving pattern contributes exactly N/2 of each.
//! A pattern that does not reach N/2 of both contributes nothing.
//! It is counted separately (`below target`), not silently dropped.
//! Every pattern that appears carries the same weight, which is what
//! makes the final usable count a sound thing to benchmark.
//!
//! `<N>` is the total verified inputs each surviving pattern contributes.
//! It splits evenly into N/2 matches and N/2 non-matches.
//! It must be a positive even integer.
//! `N = 1` is not accepted; the smallest meaningful value is 2.
//!
//! The optional `--` between N and the source name is accepted and skipped.
//! Cargo consumes its own `--` before this binary sees any argument.
//! A `--` reaching here is a literal the caller typed as a separator; it
//! has no meaning at this point.
//!
//! `--verbose` also lists every pattern excluded at generation, raw text
//! included.
//!
//! Innovatorved runs its classified test lines first, then tops up with
//! structurally generated candidates for whichever side has not yet met
//! N/2.
//! `verify_until` processes candidates in order, so the real ones fill
//! first and the top-up is reached only when the real supply was short.
//! The "from source" line below reports how much of the final corpus is
//! real versus generated for exactly this reason.
//! Every other source is unmixed: always 0 for Suricata, SpamAssassin,
//! and RegexLib; always 100% for S2ELab.

use std::path::{Path, PathBuf};

use regex_engine::data::types::SourceKind;
use regex_engine::data::run_pipeline;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // First positional is N. Missing or unparsable -> usage.
    let Some(inputs) = args.get(1).and_then(|s| s.parse::<usize>().ok()) else {
        eprintln!(
            "usage: cargo run --release --example dataset_prepare -- \
             <N> [--] <suricata|spamassassin|regexlib|s2-elab|innovatorved> \
             <file> [file...] [--corpus-dir <dir>] [--verbose]"
        );
        std::process::exit(2);
    };
    if inputs == 0 || inputs % 2 != 0 {
        eprintln!(
            "N must be a positive even integer (got {inputs}); it splits evenly into \
             N/2 matches and N/2 non-matches per pattern"
        );
        std::process::exit(2);
    }
    let per_side = inputs / 2;

    // Optional literal `--` after N, skipped.
    let mut i = 2;
    if args.get(i).map(String::as_str) == Some("--") {
        i += 1;
    }

    // Source name is required from here on.
    let Some(source_arg) = args.get(i).map(String::as_str) else {
        eprintln!(
            "usage: cargo run --release --example dataset_prepare -- \
             <N> [--] <suricata|spamassassin|regexlib|s2-elab|innovatorved> \
             <file> [file...] [--corpus-dir <dir>] [--verbose]"
        );
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
    i += 1;

    let mut files: Vec<PathBuf> = Vec::new();
    let mut corpus_dir: Option<PathBuf> = None;
    let mut verbose = false;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--corpus-dir" {
            i += 1;
            corpus_dir = args.get(i).map(PathBuf::from);
        } else if a == "--verbose" {
            verbose = true;
        } else if a == "--" {
            // A second `--` after the source name is also tolerated.
            // Cargo has already stripped its own separator, so anything
            // reaching here is a literal the caller wrote.
        } else if a.starts_with("--") {
            eprintln!("unknown argument {a:?}");
            std::process::exit(2);
        } else {
            files.push(PathBuf::from(a));
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("no input files given");
        std::process::exit(2);
    }

    let data_root = Path::new("data/processed");
    let seed = 20260101; // fixed, so re-runs reproduce the same dataset

    let report = run_pipeline(source, &files, data_root, corpus_dir.as_deref(), seed, inputs)
        .unwrap_or_else(|e| {
            eprintln!("pipeline failed: {e}");
            std::process::exit(1);
        });

    print_report(&report, source, per_side, verbose);
}

fn print_report(
    report: &regex_engine::data::PipelineReport,
    source: SourceKind,
    per_side: usize,
    verbose: bool,
) {
    let inputs = per_side * 2;
    let rule = "─".repeat(72);

    println!(
        "{:?}  --  target: {inputs} inputs per pattern ({per_side} match + {per_side} non-match)",
        source
    );
    println!("{rule}");

    println!();
    println!("Extraction");
    println!("  extracted           {:>7}", report.rules_extracted);
    println!("  unusable (frontend) {:>7}", report.patterns_unusable);
    println!(
        "  accepted (frontend) {:>7}   [{} direct, {} full-match]",
        report.patterns_accepted,
        report.patterns_unanchored_ambiguous,
        report.patterns_explicit_full_match,
    );

    println!();
    println!("Generation");
    println!(
        "  candidates generated  {:>7}",
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
    println!(
        "  candidates skipped    {:>7}   (never examined; target already reached)",
        report.inputs_skipped()
    );

    println!();
    println!("Balanced corpus");
    println!(
        "  patterns meeting target {:>7}   of {} accepted  ({per_side} match + {per_side} non-match each)",
        report.patterns_usable, report.patterns_accepted,
    );
    if report.patterns_unusable_at_generation > 0 {
        println!(
            "  patterns below target   {:>7}   (could not fill {per_side}+{per_side}, even after retry)",
            report.patterns_unusable_at_generation,
        );
    }
    println!("  inputs written          {:>7}", report.inputs_usable);
    let dropped = report.inputs_dropped_with_pattern();
    if dropped > 0 {
        println!(
            "  dropped with pattern    {:>7}   (verified, but their pattern fell below target)",
            dropped,
        );
    }

    // The two provenance lines only say something when the source
    // actually ships given input.
    // Skipped for the three generated-only sources.
    // For those, both numbers are trivially 0 and 100%.
    if report.inputs_usable_from_source > 0 || source == SourceKind::S2ELab {
        let from_source = report.inputs_usable_from_source;
        let synthetic = report.inputs_usable_synthetic();
        let pct_source = if report.inputs_usable == 0 {
            0.0
        } else {
            100.0 * from_source as f64 / report.inputs_usable as f64
        };
        let pct_synth = 100.0 - pct_source;
        println!("  from source             {:>7}   ({pct_source:>5.1}%)", from_source);
        println!("  generated to fill       {:>7}   ({pct_synth:>5.1}%)", synthetic);
    }

    println!();
    println!("  wrote  data/processed/{}/prepared.jsonl", source.dir_name());
    println!(
        "  {} patterns with {} inputs each ({} match + {} non-match)",
        report.patterns_usable, inputs, per_side, per_side,
    );

    if verbose && !report.unusable_at_generation_patterns.is_empty() {
        println!();
        println!(
            "Patterns below target ({} total):",
            report.unusable_at_generation_patterns.len()
        );
        for pattern in &report.unusable_at_generation_patterns {
            let shown: String = pattern.chars().take(100).collect();
            println!("  {shown}");
        }
    }
}