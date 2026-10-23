//! src/data/generate.rs
//!
//! Structural sampling, and S2ELab's own given examples.
//!
//! `run_pipeline` (`mod.rs`) only ever calls `given_examples` now: the
//! dataset/benchmark pipeline no longer fabricates or heuristically
//! assembles any input -- it only ever uses `data::prepare`-verified
//! copies of a source's own labeled examples. `sample_positive`,
//! `sample_negative_late`, and the `generate_*` structural-sampling
//! helpers below survive only as (a) fixture generators for
//! `prepare.rs`'s own unit tests and (b) the illustrative
//! `demo_frontend_categories` example, which shows what a translated
//! pattern's language looks like and makes no benchmark claim over the
//! samples it prints. Do not wire any of this back into `run_pipeline`.
//!
//! Every `Candidate` carries a `claimed_match` that `prepare` verifies.

use rand::rngs::StdRng;
use rand::Rng;

use crate::data::types::{Candidate, Provenance};
use crate::types::Regex;

const OUT_OF_ALPHABET_CHAR: char = '\u{2603}';
const DEFAULT_MAX_TOTAL_LEN: usize = 300;

// -------------------------------
// Structural sampling
// -------------------------------

pub fn sample_positive(r: &Regex, rng: &mut StdRng, max_star_iters: u32) -> Option<String> {
    let mut budget = DEFAULT_MAX_TOTAL_LEN;
    sample_positive_bounded(r, rng, max_star_iters, &mut budget)
}

/// Number of `Alt` leaves under `r`, flattening nested `Alt` nodes.
/// Used to weight sampling so a left-folded chain is chosen uniformly.
/// See the `Alt` case below.
fn alt_leaf_count(r: &Regex) -> usize {
    match r {
        Regex::Alt(a, b) => alt_leaf_count(a) + alt_leaf_count(b),
        _ => 1,
    }
}

fn sample_positive_bounded(
    r: &Regex,
    rng: &mut StdRng,
    max_star_iters: u32,
    budget: &mut usize,
) -> Option<String> {
    match r {
        Regex::Phi => None,
        Regex::Eps => Some(String::new()),
        Regex::Lit(c) => {
            *budget = budget.saturating_sub(1);
            Some(c.to_string())
        }
        Regex::Seq(a, b) => {
            let mut s = sample_positive_bounded(a, rng, max_star_iters, budget)?;
            s.push_str(&sample_positive_bounded(b, rng, max_star_iters, budget)?);
            Some(s)
        }
        Regex::Alt(a, b) => {
            // `alt_of_chars` builds a character class as a deeply
            // left-nested `Alt` chain.
            // A flat 50/50 choice would favour the last-inserted character.
            // Weighting by leaf count makes every leaf equally likely.
            let wa = alt_leaf_count(a) as f64;
            let wb = alt_leaf_count(b) as f64;
            if rng.gen_bool(wa / (wa + wb)) {
                sample_positive_bounded(a, rng, max_star_iters, budget)
                    .or_else(|| sample_positive_bounded(b, rng, max_star_iters, budget))
            } else {
                sample_positive_bounded(b, rng, max_star_iters, budget)
                    .or_else(|| sample_positive_bounded(a, rng, max_star_iters, budget))
            }
        }
        Regex::Star(a) => {
            let iters = rng.gen_range(0..=max_star_iters);
            let mut s = String::new();
            for _ in 0..iters {
                if *budget == 0 {
                    break;
                }
                match sample_positive_bounded(a, rng, max_star_iters, budget) {
                    Some(piece) => s.push_str(&piece),
                    None => break,
                }
            }
            Some(s)
        }
    }
}

pub fn sample_negative_late(r: &Regex, rng: &mut StdRng, max_star_iters: u32) -> Option<String> {
    let mut s = sample_positive(r, rng, max_star_iters)?;
    if s.is_empty() {
        return None;
    }
    s.pop();
    s.push(OUT_OF_ALPHABET_CHAR);
    Some(s)
}

/// A pattern can fall short of `count` distinct positives:
/// - language large enough that collisions are just unlucky (`[a-z]+` has infinitely many positives)
/// - too small to have `count` distinct members at all (`a{3}` has exactly one)
const RESAMPLE_ATTEMPTS_PER_TARGET: usize = 100;

/// Fixed `Star` iteration count for every structural sample
const SAMPLE_MAX_STAR_ITERS: u32 = 4;

/// `count` distinct positives from `r`'s own structure
pub fn generate_positives(r: &Regex, rng: &mut StdRng, count: usize) -> Vec<Candidate> {
    let mut positives: Vec<String> = Vec::new();
    for _ in 0..count * RESAMPLE_ATTEMPTS_PER_TARGET {
        if positives.len() >= count {
            break;
        }
        if let Some(text) = sample_positive(r, rng, SAMPLE_MAX_STAR_ITERS) {
            if !positives.contains(&text) {
                positives.push(text);
            }
        }
    }
    positives
        .into_iter()
        .map(|text| Candidate { text, provenance: Provenance::Structural, claimed_match: true })
        .collect()
}

/// `count` late-failing negatives from `r`'s own structure.
/// Returns fewer if the language does not admit that many.
/// See `sample_negative_late`.
pub fn generate_negatives(r: &Regex, rng: &mut StdRng, count: usize) -> Vec<Candidate> {
    let mut negatives: Vec<String> = Vec::new();
    for _ in 0..count * RESAMPLE_ATTEMPTS_PER_TARGET {
        if negatives.len() >= count {
            break;
        }
        if let Some(text) = sample_negative_late(r, rng, SAMPLE_MAX_STAR_ITERS) {
            if !negatives.contains(&text) {
                negatives.push(text);
            }
        }
    }
    negatives
        .into_iter()
        .map(|text| Candidate { text, provenance: Provenance::StructuralNegative, claimed_match: false })
        .collect()
}

/// `count` distinct positives plus up to `negative_count` late-failing
/// negatives.
/// Convenience wrapper over `generate_positives` and `generate_negatives`.
/// `count` is clamped to at least 1; use the two helpers directly if a
/// side of exactly zero is wanted.
pub fn generate_sample(
    r: &Regex,
    rng: &mut StdRng,
    count: usize,
    negative_count: usize,
) -> Vec<Candidate> {
    let count = count.max(1);
    let mut out = generate_positives(r, rng, count);
    out.extend(generate_negatives(r, rng, negative_count));
    out
}

// -------------------------------
// Given-input sources
// -------------------------------

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
    use crate::frontend::translate;
    use crate::parsers::parse_deriv_std_rec;
    use rand::SeedableRng;

    fn rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    fn core(ep_src: &str) -> Regex {
        let ep = crate::frontend::parse_ext_pattern(ep_src).expect("should parse");
        translate(&ep).expect("should translate")
    }

    #[test]
    fn sample_positive_actually_matches() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        for _ in 0..20 {
            let s = sample_positive(&r, &mut rng, 4).expect("should sample");
            assert!(
                parse_deriv_std_rec(&s, &r).is_some(),
                "sampled {:?} should match {:?}",
                s,
                r
            );
        }
    }

    #[test]
    fn sample_positive_never_returns_a_phi_string() {
        let r = Regex::Phi;
        let mut rng = rng();
        assert!(sample_positive(&r, &mut rng, 4).is_none());
    }

    #[test]
    fn sample_positive_from_a_large_class_reaches_every_member() {
        let r = core("[a-z0-9]");
        let mut rng = StdRng::seed_from_u64(99);
        let mut seen: std::collections::BTreeSet<char> = std::collections::BTreeSet::new();
        for _ in 0..20_000 {
            let s = sample_positive(&r, &mut rng, 1).expect("should sample");
            seen.insert(s.chars().next().expect("single-char sample"));
        }
        assert_eq!(
            seen.len(),
            36,
            "expected all 36 class members to be reachable, only saw {}: {:?}",
            seen.len(),
            seen
        );
    }

    #[test]
    fn sample_negative_late_fails_but_is_almost_a_match() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        for _ in 0..20 {
            let Some(s) = sample_negative_late(&r, &mut rng, 4) else { continue };
            assert!(
                parse_deriv_std_rec(&s, &r).is_none(),
                "corrupted sample {:?} should not match {:?}",
                s,
                r
            );
            let prefix_len = s.chars().count().saturating_sub(1);
            let prefix: String = s.chars().take(prefix_len).collect();
            if !prefix.is_empty() {
                assert_eq!(s.chars().count(), prefix.chars().count() + 1);
            }
        }
    }

    #[test]
    fn generate_positives_with_zero_requested_returns_empty() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        assert!(generate_positives(&r, &mut rng, 0).is_empty());
    }

    #[test]
    fn generate_negatives_with_zero_requested_returns_empty() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        assert!(generate_negatives(&r, &mut rng, 0).is_empty());
    }

    #[test]
    fn generate_positives_returns_only_positives() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        let sample = generate_positives(&r, &mut rng, 5);
        assert!(!sample.is_empty());
        assert!(sample.iter().all(|c| c.claimed_match));
        assert!(sample.iter().all(|c| c.provenance == Provenance::Structural));
    }

    #[test]
    fn generate_negatives_returns_only_negatives() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        let sample = generate_negatives(&r, &mut rng, 5);
        assert!(!sample.is_empty());
        assert!(sample.iter().all(|c| !c.claimed_match));
        assert!(sample.iter().all(|c| c.provenance == Provenance::StructuralNegative));
    }

    #[test]
    fn generate_sample_deduplicates_and_caps_at_count() {
        let r = core(r"a");
        let mut rng = rng();
        let sample = generate_sample(&r, &mut rng, 5, 0);
        assert_eq!(sample.iter().filter(|c| c.claimed_match).count(), 1);
        assert_eq!(sample[0].text, "a");
    }

    #[test]
    fn generate_sample_returns_up_to_count_distinct_variants() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        let sample = generate_sample(&r, &mut rng, 5, 0);
        assert!(!sample.is_empty());
        assert!(sample.len() <= 5);
        let distinct: std::collections::HashSet<_> = sample.iter().map(|c| &c.text).collect();
        assert_eq!(distinct.len(), sample.len(), "no duplicate texts");
        assert!(sample.iter().all(|c| c.claimed_match));
    }

    #[test]
    fn generate_sample_returns_up_to_count_of_each_kind() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        let sample = generate_sample(&r, &mut rng, 5, 5);
        let positives = sample.iter().filter(|c| c.provenance == Provenance::Structural).count();
        let negatives = sample.iter().filter(|c| c.provenance == Provenance::StructuralNegative).count();
        assert!(positives >= 1 && positives <= 5);
        assert!(negatives >= 1 && negatives <= 5);
    }

    #[test]
    fn generate_sample_with_zero_negatives_requested_returns_none() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        let sample = generate_sample(&r, &mut rng, 5, 0);
        assert!(sample.iter().all(|c| c.provenance != Provenance::StructuralNegative));
    }

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