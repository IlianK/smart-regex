//! examples/demo_frontend_categories.rs
//!
//! cargo run --release --example demo_frontend_categories -- <file...> [--seed N] [--diag 0|1|2|3]
//!
//! For each verified category of frontend behaviour 
//! (flags, class/escape handling, outright rejections, the two faithfulness gaps), 
//! finds a REAL pattern in the given corpus files that exercises it, then shows the actual behaviour: 
//! - translated Regex and a few traces, 
//! - the exact rejection
//! - the error or gap
//! 
//! A category with no real example in the given files is reported as "no example found" and skipped
//!
//! Files are classified by extension: 
//! `.rules` -> Suricata, `.cf` -> SpamAssassin, anything else -> RegexLib. 
//! `--seed N` makes candidate generation deterministic; 
//! `--diag 0|1|2|3` sets trace verbosity (default 1).

use std::collections::HashSet;
use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::SeedableRng;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::generate::{generate_best, generate_neutral, generate_worst_structural};
use regex_engine::data::prepare::core_regex;
use regex_engine::data::types::{Candidate, Category, Provenance, SourceKind};
use regex_engine::diagnostics::{run_parser, DiagConfig, DiagLevel};
use regex_engine::frontend::{
    detect_anchors, faithfulness_gaps, is_faithful, parse_ext_pattern, parse_pcre_rule,
    strip_pcre_delimiters, ExtPat, FaithfulnessGap,
};
use regex_engine::parsers::ParserType;
use regex_engine::types::Regex;

fn size_regex(r: &Regex) -> usize {
    match r {
        Regex::Phi | Regex::Eps | Regex::Lit(_) => 1,
        Regex::Alt(a, b) | Regex::Seq(a, b) => 1 + size_regex(a) + size_regex(b),
        Regex::Star(a) => 1 + size_regex(a),
    }
}

fn invert_ascii_case(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

fn contains_dot(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Dot => true,
        ExtPat::Or(parts) | ExtPat::Concat(parts) => parts.iter().any(contains_dot),
        ExtPat::Group(inner)
        | ExtPat::GroupNonMarking(inner)
        | ExtPat::Opt(inner)
        | ExtPat::Plus(inner)
        | ExtPat::Star(inner)
        | ExtPat::Bound(inner, ..) => contains_dot(inner),
        _ => false,
    }
}

/// Textual scan for a backslash-octal escape inside a `[...]` class; not a
/// full PCRE parse, just enough to pick one real example from the corpus.
fn has_octal_in_class(body: &str) -> bool {
    let chars: Vec<char> = body.chars().collect();
    let mut in_class = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '\\' if in_class && chars.get(i + 1).is_some_and(|c| ('0'..='7').contains(c)) => {
                return true
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Textual scan for a bare `\0` outside any `[...]` class.
fn has_bare_backslash_zero(body: &str) -> bool {
    let chars: Vec<char> = body.chars().collect();
    let mut in_class = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '\\' if !in_class && chars.get(i + 1) == Some(&'0') => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

fn accepted_and_faithful(p: &str) -> bool {
    parse_pcre_rule(p).is_ok() && is_faithful(p)
}

fn rejected_because(p: &str, needle: &str) -> bool {
    matches!(parse_pcre_rule(p), Err(e) if e.contains(needle))
}

// -------------------------------
// Category detectors
// -------------------------------

fn is_case_insensitive(p: &str) -> bool {
    accepted_and_faithful(p) && strip_pcre_delimiters(p).1.case_insensitive
}

fn is_dot_without_s(p: &str) -> bool {
    if !accepted_and_faithful(p) {
        return false;
    }
    let (body, flags) = strip_pcre_delimiters(p);
    !flags.dot_all && parse_ext_pattern(body).map(|ep| contains_dot(&ep)).unwrap_or(false)
}

fn is_dot_with_s(p: &str) -> bool {
    if !accepted_and_faithful(p) {
        return false;
    }
    let (body, flags) = strip_pcre_delimiters(p);
    flags.dot_all && parse_ext_pattern(body).map(|ep| contains_dot(&ep)).unwrap_or(false)
}

fn is_anchored_flag(p: &str) -> bool {
    accepted_and_faithful(p) && strip_pcre_delimiters(p).1.anchored
}

fn is_dollar_lenient_default(p: &str) -> bool {
    if !accepted_and_faithful(p) || strip_pcre_delimiters(p).1.dollar_endonly {
        return false;
    }
    matches!(detect_anchors(p), Ok((_, true)))
}

fn is_dollar_endonly_flag(p: &str) -> bool {
    if !accepted_and_faithful(p) || !strip_pcre_delimiters(p).1.dollar_endonly {
        return false;
    }
    matches!(detect_anchors(p), Ok((_, true)))
}

/// Real `[:name:]` POSIX class, rejected outright. Checking the rejection
/// reason (not a `[:` substring test) matters: `[:*:]` contains that text
/// but is an ordinary class of three literals, and is accepted.
fn is_posix_class_rejected(p: &str) -> bool {
    rejected_because(p, "POSIX class")
}

fn is_octal_in_class(p: &str) -> bool {
    accepted_and_faithful(p) && has_octal_in_class(strip_pcre_delimiters(p).0)
}

fn is_bare_backslash_zero(p: &str) -> bool {
    accepted_and_faithful(p) && has_bare_backslash_zero(strip_pcre_delimiters(p).0)
}

fn is_multiline_rejected(p: &str) -> bool {
    strip_pcre_delimiters(p).1.multiline && rejected_because(p, "multiline")
}

fn is_extended_rejected(p: &str) -> bool {
    strip_pcre_delimiters(p).1.extended && rejected_because(p, "extended")
}

fn is_backreference_rejected(p: &str) -> bool {
    rejected_because(p, "backreference")
}

fn is_lookahead_rejected(p: &str) -> bool {
    rejected_because(p, "lookahead")
}

fn is_word_boundary_rejected(p: &str) -> bool {
    rejected_because(p, "word boundary")
}

fn is_relative_gap(p: &str) -> bool {
    parse_pcre_rule(p).is_ok() && faithfulness_gaps(p).contains(&FaithfulnessGap::Relative)
}

fn is_nested_anchor_gap(p: &str) -> bool {
    parse_pcre_rule(p).is_ok() && faithfulness_gaps(p).contains(&FaithfulnessGap::NestedAnchor)
}

// -------------------------------
// Shared candidate machinery (same generators and wrapping as
// examples/demo_anchored_samples.rs)
// -------------------------------

fn describe(candidate: &Candidate) -> &'static str {
    match (candidate.category, candidate.provenance, candidate.claimed_match) {
        (Category::Best, ..) => "shortest matching input",
        (Category::Neutral, ..) => "typical matching input (moderate repetition)",
        (Category::Worst, Provenance::Structural, true) => {
            "hardest matching input (maximal repetition)"
        }
        (Category::Worst, Provenance::StructuralNegative, false) => {
            "near-miss: corrupted late, must NOT match"
        }
        _ => "generated input",
    }
}

fn generate_inputs(core: &Regex, rng: &mut StdRng) -> Vec<Candidate> {
    let mut inputs: Vec<Candidate> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut absorb = |cands: Vec<Candidate>, inputs: &mut Vec<Candidate>| {
        for c in cands {
            if seen.insert(c.text.clone()) {
                inputs.push(c);
            }
        }
    };

    absorb(generate_best(core, rng, 2), &mut inputs);
    absorb(generate_neutral(core, rng, 2), &mut inputs);
    absorb(generate_worst_structural(core, rng, 6, 1), &mut inputs);

    inputs.truncate(3);
    inputs
}

fn noise(rng: &mut StdRng, len: usize) -> String {
    use rand::seq::SliceRandom;
    let chars = regex_engine::frontend::alphabet::alphabet();
    (0..len).map(|_| *chars.choose(rng).expect("alphabet() is non-empty")).collect()
}

/// Same purpose as `demo_anchored_samples::wrap_for_anchor_shape`: a
/// candidate drawn straight from the core starts and ends exactly where
/// the core does, so a partially-anchored pattern's padding is never
/// actually exercised unless noise is added on the padded side.
fn wrap_for_anchor_shape(text: &str, start_anchored: bool, end_anchored: bool, rng: &mut StdRng) -> String {
    match (start_anchored, end_anchored) {
        (true, true) => text.to_string(),
        (true, false) => format!("{text}{}", noise(rng, 4)),
        (false, true) => format!("{}{text}", noise(rng, 4)),
        (false, false) => format!("{}{text}{}", noise(rng, 4), noise(rng, 4)),
    }
}

// -------------------------------
// Demo kinds
// -------------------------------

/// Generic accepted pattern: core/padded sizes, then up to 3 generated
/// candidates traced against the real padded regex.
fn demo_accepted(pattern: &str, rng: &mut StdRng, diag: DiagLevel) {
    let core = core_regex(pattern).expect("category detector already required acceptance");
    let padded = parse_pcre_rule(pattern).expect("category detector already required acceptance");
    let (start, end) = detect_anchors(pattern).expect("category detector already required acceptance");

    println!(
        "Translation: core {} nodes -> search-padded {} nodes \
         (data::prepare::core_regex, frontend::parse_pcre_rule)",
        size_regex(&core),
        size_regex(&padded)
    );
    println!(
        "Anchors: {}",
        match (start, end) {
            (true, true) => "^...$ (no padding either side)",
            (true, false) => "^... (padded on the right)",
            (false, true) => "...$ (padded on the left)",
            (false, false) => "... (padded on both sides)",
        }
    );

    let candidates = generate_inputs(&core, rng);
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);
    println!("Generated {} input(s) against the padded regex:", candidates.len());
    for c in &candidates {
        let text = wrap_for_anchor_shape(&c.text, start, end, rng);
        let expect = if c.claimed_match { "MATCH" } else { "NO MATCH" };
        println!("{}", "-".repeat(70));
        println!("  [expected {expect}] {:?}  -- {}", text, describe(c));
        println!("{}", "-".repeat(70));
        run_parser(pattern, &padded, &text, &config);
        println!();
    }
}

/// Targeted extra check: flips the ASCII case of every letter in the
/// shortest match and shows it still matches -- isolates what `case_fold`
/// buys, rather than hoping a generic candidate differs in case.
fn extra_case_insensitive(pattern: &str, rng: &mut StdRng, diag: DiagLevel) {
    let core = core_regex(pattern).expect("category detector already required acceptance");
    let padded = parse_pcre_rule(pattern).expect("category detector already required acceptance");
    let Some(best) = generate_best(&core, rng, 1).into_iter().next() else {
        println!("extra check: no generated input available, skipped");
        return;
    };
    let flipped = invert_ascii_case(&best.text);
    println!("Extra check (case invariance): {:?} case-flipped to {:?}", best.text, flipped);
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);
    println!("  expected MATCH:");
    run_parser(pattern, &padded, &flipped, &config);
}

/// Targeted extra check: appends a trailing '\n' and shows it still matches,
/// isolating the default `$` leniency `translate_as_search` builds in.
fn extra_dollar_lenient(pattern: &str, rng: &mut StdRng, diag: DiagLevel) {
    let core = core_regex(pattern).expect("category detector already required acceptance");
    let padded = parse_pcre_rule(pattern).expect("category detector already required acceptance");
    let Some(best) = generate_best(&core, rng, 1).into_iter().next() else {
        println!("extra check: no generated input available, skipped");
        return;
    };
    let with_newline = format!("{}\n", best.text);
    println!("Extra check (lenient '$'): {:?} with one trailing '\\n' appended", best.text);
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);
    println!("  expected MATCH (one trailing '\\n' is allowed without 'E'):");
    run_parser(pattern, &padded, &with_newline, &config);
}

/// Symmetric counterpart for a real `/E` pattern: the same trailing '\n'
/// must now be rejected, since 'E' makes '$' strict.
fn extra_dollar_strict(pattern: &str, rng: &mut StdRng, diag: DiagLevel) {
    let core = core_regex(pattern).expect("category detector already required acceptance");
    let padded = parse_pcre_rule(pattern).expect("category detector already required acceptance");
    let Some(best) = generate_best(&core, rng, 1).into_iter().next() else {
        println!("extra check: no generated input available, skipped");
        return;
    };
    let with_newline = format!("{}\n", best.text);
    println!("Extra check (strict '$'): {:?} with one trailing '\\n' appended", best.text);
    let config = DiagConfig::new(diag, ParserType::DerivStdRec, None);
    println!("  expected NO MATCH ('E' forbids a trailing '\\n'):");
    run_parser(pattern, &padded, &with_newline, &config);
}

fn demo_rejected(pattern: &str) {
    let err = parse_pcre_rule(pattern).expect_err("category detector already required rejection");
    println!("frontend::parse_pcre_rule(..) = Err({err:?})");
}

/// Accepted but not faithful: prints each gap and what it costs, then runs
/// the generic accepted demo on the same pattern.
fn demo_gap(pattern: &str, rng: &mut StdRng, diag: DiagLevel) {
    let gaps = faithfulness_gaps(pattern);
    println!("frontend::parse_pcre_rule(..) = Ok(..) -- accepted, but:");
    for gap in &gaps {
        match gap {
            FaithfulnessGap::Relative => println!(
                "  FaithfulnessGap::Relative -- the 'R' flag means \"relative to the \
                 previous content match\", a positioning constraint the frontend cannot \
                 see from the pattern string alone. It is translated as an ordinary \
                 search pattern instead."
            ),
            FaithfulnessGap::NestedAnchor => println!(
                "  FaithfulnessGap::NestedAnchor -- a '^' or '$' appears somewhere in the \
                 pattern that is not a real top-level anchor (or not present on every \
                 branch of the alternation it sits in). translate() silently lowers it \
                 to Eps rather than enforcing or rejecting it."
            ),
        }
    }
    println!("is_faithful(..) = {}", is_faithful(pattern));
    demo_accepted(pattern, rng, diag);
}

struct Entry {
    name: &'static str,
    detect: fn(&str) -> bool,
    kind: Kind,
    extra: Option<fn(&str, &mut StdRng, DiagLevel)>,
}

enum Kind {
    Accepted,
    Rejected,
    Gap,
}

fn entries() -> Vec<Entry> {
    vec![
        Entry {
            name: "case-insensitive ('i' flag)",
            detect: is_case_insensitive,
            kind: Kind::Accepted,
            extra: Some(extra_case_insensitive),
        },
        Entry {
            name: "'.' excludes '\\n' (no 's' flag)",
            detect: is_dot_without_s,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "'.' includes '\\n' ('s' flag)",
            detect: is_dot_with_s,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "start-anchored ('A' flag)",
            detect: is_anchored_flag,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "'$' allows one trailing '\\n' (default, no 'E' flag)",
            detect: is_dollar_lenient_default,
            kind: Kind::Accepted,
            extra: Some(extra_dollar_lenient),
        },
        Entry {
            name: "'$' matches only at true end ('E' flag)",
            detect: is_dollar_endonly_flag,
            kind: Kind::Accepted,
            extra: Some(extra_dollar_strict),
        },
        Entry {
            name: "octal escape inside a class",
            detect: is_octal_in_class,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "bare '\\0' outside a class",
            detect: is_bare_backslash_zero,
            kind: Kind::Accepted,
            extra: None,
        },
        Entry {
            name: "rejected: multiline ('m' flag)",
            detect: is_multiline_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: extended ('x' flag)",
            detect: is_extended_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: backreference",
            detect: is_backreference_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: lookahead",
            detect: is_lookahead_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: word boundary",
            detect: is_word_boundary_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "rejected: POSIX class (e.g. [:alpha:])",
            detect: is_posix_class_rejected,
            kind: Kind::Rejected,
            extra: None,
        },
        Entry {
            name: "gap: relative ('R' flag)",
            detect: is_relative_gap,
            kind: Kind::Gap,
            extra: None,
        },
        Entry {
            name: "gap: nested anchor",
            detect: is_nested_anchor_gap,
            kind: Kind::Gap,
            extra: None,
        },
    ]
}

fn classify_file(path: &PathBuf) -> SourceKind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rules") => SourceKind::Suricata,
        Some("cf") => SourceKind::SpamAssassin,
        _ => SourceKind::RegexLib,
    }
}

fn load_all_patterns(files: &[PathBuf]) -> Vec<String> {
    let mut patterns = Vec::new();
    for path in files {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {}: {e}", path.display());
                continue;
            }
        };
        let extracted = match classify_file(path) {
            SourceKind::Suricata => extract_suricata(&text),
            SourceKind::SpamAssassin => extract_spamassassin(&text),
            SourceKind::RegexLib => extract_regexlib(&text),
        };
        patterns.extend(extracted.into_iter().map(|r| r.raw_pattern));
    }
    patterns
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: cargo run --release --example demo_frontend_categories -- \
             <file> [file...] [--seed N] [--diag 0|1|2|3]"
        );
        std::process::exit(2);
    }

    let mut seed: Option<u64> = None;
    let mut diag = DiagLevel::Basic;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                i += 1;
                let Some(s) = args.get(i).and_then(|s| s.parse().ok()) else {
                    eprintln!("--seed needs a number");
                    std::process::exit(2);
                };
                seed = Some(s);
            }
            "--diag" => {
                i += 1;
                diag = match args.get(i).map(String::as_str) {
                    Some("0") => DiagLevel::Off,
                    Some("1") => DiagLevel::Basic,
                    Some("2") => DiagLevel::Verbose,
                    Some("3") => DiagLevel::Debug,
                    _ => {
                        eprintln!("--diag needs 0, 1, 2, or 3");
                        std::process::exit(2);
                    }
                };
            }
            other => files.push(PathBuf::from(other)),
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("no dataset files given");
        std::process::exit(2);
    }

    let patterns = load_all_patterns(&files);
    println!("{} raw patterns extracted from {} file(s)\n", patterns.len(), files.len());

    let mut rng = match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    };

    for entry in entries() {
        println!("{}", "=".repeat(70));
        println!("{}", entry.name);
        println!("{}", "=".repeat(70));

        // Deterministic selection (shortest real example), independent of
        // --seed: --seed only controls candidate generation below.
        let found = patterns.iter().filter(|p| (entry.detect)(p)).min_by_key(|p| p.len());

        let Some(pattern) = found else {
            println!("no example found in this dataset -- skipped\n");
            continue;
        };

        println!("Raw pattern: {pattern}\n");
        match entry.kind {
            Kind::Accepted => demo_accepted(pattern, &mut rng, diag),
            Kind::Rejected => demo_rejected(pattern),
            Kind::Gap => demo_gap(pattern, &mut rng, diag),
        }
        if let Some(extra) = entry.extra {
            println!();
            extra(pattern, &mut rng, diag);
        }
        println!();
    }
}