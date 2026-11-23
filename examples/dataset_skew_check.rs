//! examples/dataset_skew_check.rs
//!
//! One-off analysis: for every raw S2ELab pattern, record its final
//! pipeline status (duplicate / rejected / accepted-anchored /
//! accepted-unanchored) alongside its *raw* (pre-filtering) match and
//! non-match counts and lengths, so the rejected and accepted subsets'
//! input characteristics can be compared directly. Not part of the
//! regular pipeline; written to answer one specific thesis question.
//!
//! cargo run --release --example dataset_skew_check -- data/raw/s2-elab/RegexEval.json

use std::collections::HashSet;
use std::path::PathBuf;

use regex_engine::data::extract::extract_s2_elab;
use regex_engine::data::prepare::core_regex_tagged;
use regex_engine::data::types::{MatchIntent, RuleContext};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = args.get(1).cloned().unwrap_or_else(|| "data/raw/s2-elab/RegexEval.json".to_string());
    let text = std::fs::read_to_string(&file).expect("read raw S2ELab json");
    let rules = extract_s2_elab(&text);

    println!(
        "id,pattern_len,status,reason,n_matches,n_non_matches,avg_match_len,max_match_len,avg_nonmatch_len,max_nonmatch_len"
    );

    let mut seen: HashSet<&str> = HashSet::new();
    for (idx, rule) in rules.iter().enumerate() {
        let (matches, non_matches) = match &rule.context {
            RuleContext::S2ELab { matches, non_matches } => (matches.as_slice(), non_matches.as_slice()),
            _ => (&[][..], &[][..]),
        };
        let n_matches = matches.len();
        let n_non = non_matches.len();
        let avg_match_len = mean_len(matches);
        let max_match_len = max_len(matches);
        let avg_nonmatch_len = mean_len(non_matches);
        let max_nonmatch_len = max_len(non_matches);
        let pattern_len = rule.raw_pattern.chars().count();

        if !seen.insert(rule.raw_pattern.as_str()) {
            println!(
                "{idx},{pattern_len},duplicate,,{n_matches},{n_non},{avg_match_len:.2},{max_match_len},{avg_nonmatch_len:.2},{max_nonmatch_len}"
            );
            continue;
        }

        match core_regex_tagged(&rule.raw_pattern) {
            Ok((_, MatchIntent::ExplicitFullMatch)) => {
                println!(
                    "{idx},{pattern_len},accepted,anchored,{n_matches},{n_non},{avg_match_len:.2},{max_match_len},{avg_nonmatch_len:.2},{max_nonmatch_len}"
                );
            }
            Ok((_, MatchIntent::UnanchoredAmbiguous)) => {
                println!(
                    "{idx},{pattern_len},accepted,unanchored,{n_matches},{n_non},{avg_match_len:.2},{max_match_len},{avg_nonmatch_len:.2},{max_nonmatch_len}"
                );
            }
            Err(reason) => {
                let short_reason = classify_reason(&reason);
                println!(
                    "{idx},{pattern_len},rejected,{short_reason},{n_matches},{n_non},{avg_match_len:.2},{max_match_len},{avg_nonmatch_len:.2},{max_nonmatch_len}"
                );
            }
        }
    }
    let _ = PathBuf::from(&file);
}

fn mean_len(items: &[String]) -> f64 {
    if items.is_empty() {
        return 0.0;
    }
    items.iter().map(|s| s.chars().count()).sum::<usize>() as f64 / items.len() as f64
}

fn max_len(items: &[String]) -> usize {
    items.iter().map(|s| s.chars().count()).max().unwrap_or(0)
}

/// Coarse reason bucket from `core_regex_tagged`'s error text, matching
/// the categories already reported in the thesis's rejection-cause table.
fn classify_reason(err: &str) -> &'static str {
    let e = err.to_lowercase();
    if e.contains("lookaround") || e.contains("lookahead") || e.contains("lookbehind") {
        "lookaround"
    } else if e.contains("backreference") {
        "backreference"
    } else if e.contains("word boundary") || e.contains("\\b") {
        "word_boundary"
    } else if e.contains("anchor") {
        "anchor"
    } else if e.contains("node") {
        "node_cap"
    } else if e.contains("depth") {
        "depth_cap"
    } else {
        "other"
    }
}
