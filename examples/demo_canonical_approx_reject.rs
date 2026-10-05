//! examples/demo_canonical_approx_reject.rs
//!
//! cargo run --release --example demo_canonical_approx_reject [-- <file...>]
//!
//! Example per category `dataset_stats.rs` counts:
//! - Two Approx causes (R flag, nested anchor) 
//! - Six Reject causes (backreference, lookaround, word-boundary, m flag, x flag, other). 

use std::path::PathBuf;

use regex_engine::data::extract::{extract_regexlib, extract_spamassassin, extract_suricata};
use regex_engine::data::types::SourceKind;
use regex_engine::frontend::{faithfulness_gaps, parse_pcre_rule, FaithfulnessGap};

// ---------------------------------------------------------------------
// Corpus loading -- same default layout as dataset_stats.rs
// ---------------------------------------------------------------------

fn default_dir(source: SourceKind) -> &'static str {
    match source {
        SourceKind::Suricata => "data/raw/snort",
        SourceKind::SpamAssassin => "data/raw/spamAssassin",
        SourceKind::RegexLib => "data/raw/regexLib",
    }
}

fn files_in(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn classify_file(path: &PathBuf) -> SourceKind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rules") => SourceKind::Suricata,
        Some("cf") => SourceKind::SpamAssassin,
        _ => SourceKind::RegexLib,
    }
}

/// (pattern, source name) for every extracted pattern in `files`.
fn load_patterns(files: &[(PathBuf, SourceKind)]) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for (path, source) in files {
        let text = match std::fs::read(path) {
            Ok(bytes) => String::from_utf8(bytes.clone())
                .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned()),
            Err(e) => {
                eprintln!("skipping {}: {e}", path.display());
                continue;
            }
        };
        let extracted = match source {
            SourceKind::Suricata => extract_suricata(&text),
            SourceKind::SpamAssassin => extract_spamassassin(&text),
            SourceKind::RegexLib => extract_regexlib(&text),
        };
        for rule in extracted {
            out.push((rule.raw_pattern, source.dir_name()));
        }
    }
    out
}

/// Shortest pattern for which `detect` holds, or `None` if the corpus has
/// no example. Deterministic: doesn't depend on extraction order.
fn shortest_matching<'a>(
    patterns: &'a [(String, &'static str)],
    detect: impl Fn(&str) -> bool,
) -> Option<&'a (String, &'static str)> {
    patterns.iter().filter(|(p, _)| detect(p)).min_by_key(|(p, _)| p.len())
}

fn rejected_because(p: &str, needle: &str) -> bool {
    matches!(parse_pcre_rule(p), Err(e) if e.to_lowercase().contains(needle))
}

fn gap(p: &str, gap: FaithfulnessGap) -> bool {
    parse_pcre_rule(p).is_ok() && faithfulness_gaps(p).contains(&gap)
}

// ---------------------------------------------------------------------
// Reject: one section per Cause, mirroring dataset_stats.rs's Cause enum
// ---------------------------------------------------------------------

struct RejectCause {
    label: &'static str,
    detect: fn(&str) -> bool,
}

const REJECT_CAUSES: &[RejectCause] = &[
    RejectCause { label: "backreference", detect: |p| rejected_because(p, "backreference") },
    RejectCause { label: "lookaround", detect: |p| rejected_because(p, "lookahead") || rejected_because(p, "lookbehind") },
    RejectCause { label: "word-boundary", detect: |p| rejected_because(p, "word boundary") },
    RejectCause { label: "flag-m (multiline)", detect: |p| rejected_because(p, "multiline") },
    RejectCause { label: "flag-x (extended)", detect: |p| rejected_because(p, "extended") },
    RejectCause {
        label: "other",
        detect: |p| {
            matches!(parse_pcre_rule(p), Err(e) if {
                let e = e.to_lowercase();
                !e.contains("backreference") && !e.contains("lookahead") && !e.contains("lookbehind")
                    && !e.contains("word boundary") && !e.contains("multiline") && !e.contains("extended")
            })
        },
    },
];

fn show_reject(patterns: &[(String, &'static str)]) {
    println!("{}", "=".repeat(74));
    println!("REJECT: no Regex produced at all, no approximation attempted");
    println!("{}", "=".repeat(74));
    println!(
        "dataset_stats.rs's six Reject causes."
    );

    for cause in REJECT_CAUSES {
        println!();
        println!("--- {} ---", cause.label);
        match shortest_matching(patterns, cause.detect) {
            None => println!("(no example in this corpus)"),
            Some((pattern, source)) => {
                let err = parse_pcre_rule(pattern).expect_err("detector already required rejection");
                println!("source:  {source}");
                println!("pattern: {pattern}");
                println!("frontend::parse_pcre_rule(..) = Err({err:?})");
            }
        }
    }
}

// ---------------------------------------------------------------------
// Approx: one section per Caveat that actually downgrades a pattern
// ---------------------------------------------------------------------

fn show_approx_relative(patterns: &[(String, &'static str)]) {
    println!();
    println!("{}", "=".repeat(74));
    println!("APPROX: R (relative) -- Suricata's \"relative to the previous content:\n\
               match\" positioning hint");
    println!("{}", "=".repeat(74));
    println!(
        "R` is translated as an ordinary (unanchored) search\n\
         pattern instead of a position constraint: the approximation is simply to\n\
         drop the relative-positioning requirement."
    );

    match shortest_matching(patterns, |p| gap(p, FaithfulnessGap::Relative)) {
        None => println!("\n(no real R example in this corpus)"),
        Some((pattern, source)) => {
            println!("\nReal example:");
            println!("source:  {source}");
            println!("pattern: {pattern}");
            let r = parse_pcre_rule(pattern).expect("gap detector already required acceptance");
            println!("accepted: frontend::parse_pcre_rule(..) = Ok(..)");
            println!("faithfulness_gaps(..) = {:?}", faithfulness_gaps(pattern));

            // Minimal illustration, alongside the real (longer) pattern above:
            // with vs. without R translate identically, because R carries no
            // information this frontend ever reads.
            let with_r = "/abc/R";
            let without_r = "/abc/";
            let rt = parse_pcre_rule(with_r).unwrap();
            let rf = parse_pcre_rule(without_r).unwrap();
            println!(
                "\nMinimal illustration: {with_r:?} and {without_r:?} translate to the \
                 same search regex."
            );
            for probe in ["abc", "xabcx", "abcx"] {
                let mt = regex_engine::parsers::parse_deriv_std_rec(probe, &rt).is_some();
                let mf = regex_engine::parsers::parse_deriv_std_rec(probe, &rf).is_some();
                println!("  {:?} -- with R: {mt}, without R: {mf} (identical)", probe);
            }
            let _ = r;
        }
    }
}

fn show_approx_nested_anchor(patterns: &[(String, &'static str)]) {
    println!();
    println!("{}", "=".repeat(74));
    println!("APPROX: nested anchor -- '^'/'$' present, but not a real top-level\n\
               anchor on every branch");
    println!("{}", "=".repeat(74));
    println!(
        "`translate` lowers an anchor that isn't in a top-level position\n\
         (or isn't on every branch of the alternation it sits in) to Eps.
         The approximation is to ignore\n\
         the anchor, never to reject the pattern."
    );

    match shortest_matching(patterns, |p| gap(p, FaithfulnessGap::NestedAnchor)) {
        None => println!("\n(no real nested-anchor example in this corpus)"),
        Some((pattern, source)) => {
            println!("\nReal example:");
            println!("source:  {source}");
            println!("pattern: {pattern}");
            println!("accepted: frontend::parse_pcre_rule(..) = Ok(..)");
            println!("faithfulness_gaps(..) = {:?}", faithfulness_gaps(pattern));

            // Minimal illustration: `^` guards only the left branch of a
            // top-level Or, so it is structurally "not on every branch" and
            // gets lowered to Eps -- the engine matches "bc" even though a
            // real nested-anchor-respecting reading of `^` would not.
            let minimal = "(a|^b)c";
            let r = parse_pcre_rule(&format!("/{minimal}/")).unwrap();
            let matched = regex_engine::parsers::parse_deriv_std_rec("bc", &r).is_some();
            println!(
                "\nMinimal illustration: {minimal:?} against \"bc\" (`^` only guards the\n\
                 left branch `a`, not `b`, so it is a nested anchor, not a top-level one):"
            );
            println!(
                "  matches \"bc\": {matched} -- the '^' before 'b' is lowered to Eps, so \
                 'b' is reachable from any position, exactly as if the '^' were not there."
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let files: Vec<(PathBuf, SourceKind)> = if args.is_empty() {
        [SourceKind::Suricata, SourceKind::SpamAssassin, SourceKind::RegexLib]
            .into_iter()
            .flat_map(|s| files_in(default_dir(s)).into_iter().map(move |p| (p, s)))
            .collect()
    } else {
        args.iter().map(PathBuf::from).map(|p| { let s = classify_file(&p); (p, s) }).collect()
    };

    if files.is_empty() {
        eprintln!("no corpus files found (looked under data/raw/{{snort,spamAssassin,regexLib}})");
        std::process::exit(2);
    }

    let patterns = load_patterns(&files);
    println!("{} raw patterns extracted from {} file(s)", patterns.len(), files.len());

    show_reject(&patterns);
    show_approx_relative(&patterns);
    show_approx_nested_anchor(&patterns);
}
