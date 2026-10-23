//! src/data/generate.rs
//!
//! `run_pipeline` (`mod.rs`) only ever uses a source's own given,
//! pre-labeled match/non-match examples -- today that means S2ELab's
//! `matches`/`non_matches`. No input is generated, derived, or
//! heuristically assembled anywhere in this pipeline.
//!
//! Every `Candidate` carries a `claimed_match` that `prepare` verifies.

use crate::data::types::{Candidate, Provenance};

/// S2ELab: wrap the pattern author's own `matches`/`non_matches` as
/// candidates, unchanged, unsampled, and uncapped -- every given
/// example becomes a candidate.
/// Duplicate strings are kept once.
/// A string in both lists is kept only as a positive, the stronger claim.
pub fn given_examples(matches: &[String], non_matches: &[String]) -> Vec<Candidate> {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for text in matches {
        if seen.insert(text.as_str()) {
            out.push(Candidate {
                text: text.clone(),
                provenance: Provenance::DatasetGiven,
                claimed_match: true,
            });
        }
    }
    for text in non_matches {
        if seen.insert(text.as_str()) {
            out.push(Candidate {
                text: text.clone(),
                provenance: Provenance::DatasetGiven,
                claimed_match: false,
            });
        }
    }
    out
}

// -------------------------------
// Tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_examples_wrap_matches_and_non_matches_unchanged() {
        let matches = vec!["12345".to_string(), "00000".to_string()];
        let non_matches = vec!["abcde".to_string()];
        let candidates = given_examples(&matches, &non_matches);
        assert_eq!(candidates.len(), 3);
        let positives: Vec<_> = candidates.iter().filter(|c| c.claimed_match).collect();
        let negatives: Vec<_> = candidates.iter().filter(|c| !c.claimed_match).collect();
        assert_eq!(positives.len(), 2);
        assert_eq!(negatives.len(), 1);
        assert!(candidates.iter().all(|c| c.provenance == Provenance::DatasetGiven));
        assert_eq!(negatives[0].text, "abcde");
    }

    #[test]
    fn given_examples_are_not_capped() {
        let matches: Vec<String> = (0..100).map(|i| format!("m{i}")).collect();
        let non_matches: Vec<String> = (0..100).map(|i| format!("n{i}")).collect();
        let candidates = given_examples(&matches, &non_matches);
        assert_eq!(candidates.len(), 200);
        assert_eq!(candidates.iter().filter(|c| c.claimed_match).count(), 100);
        assert_eq!(candidates.iter().filter(|c| !c.claimed_match).count(), 100);
    }

    #[test]
    fn given_examples_keep_a_string_in_both_lists_only_as_a_positive() {
        let matches = vec!["x".to_string()];
        let non_matches = vec!["x".to_string(), "y".to_string()];
        let candidates = given_examples(&matches, &non_matches);
        assert_eq!(candidates.len(), 2);
        let x = candidates.iter().find(|c| c.text == "x").expect("x present");
        assert!(x.claimed_match);
    }

    #[test]
    fn given_examples_with_nothing_given_yields_nothing() {
        assert!(given_examples(&[], &[]).is_empty());
    }
}
