//! src/data/generate.rs
//!
//! Per-source input generation:
//!
//! - Snort/Suricata: assemble candidates from the rule's own `content:`
//!   fields (real evidence of what a triggering payload contains).
//! - SpamAssassin: sample real messages from a local ham/spam corpus,
//!   when one is configured.
//! - RegexLib (and the fallback for the other two): walk the pattern's
//!   own structure to produce a sample.
//!
//! Every `Candidate` carries a `claimed_match` that `prepare` verifies.

use std::path::Path;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::data::types::{Candidate, ContentField, Provenance};
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
/// Used to weight sampling so a left-folded chain is chosen uniformly
/// (see the `Alt` case below).
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
            // A flat 50/50 choice would heavily favour the last-inserted
            // character; weighting by leaf count makes every leaf equally
            // likely regardless of tree shape.
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

/// Fixed `Star` iteration count for every structural sample.
/// Enough repetition to be realistic without adversarial growth.
const SAMPLE_MAX_STAR_ITERS: u32 = 4;

/// `count` distinct positives sampled from `r`'s own structure.
/// Plus up to `negative_count` late-failing negatives
/// (see `sample_negative_late`) so rejection is exercised too.
pub fn generate_sample(
    r: &Regex,
    rng: &mut StdRng,
    count: usize,
    negative_count: usize,
) -> Vec<Candidate> {
    let count = count.max(1);

    let mut positives: Vec<String> = Vec::new();
    for _ in 0..count * 4 {
        if positives.len() >= count {
            break;
        }
        if let Some(text) = sample_positive(r, rng, SAMPLE_MAX_STAR_ITERS) {
            if !positives.contains(&text) {
                positives.push(text);
            }
        }
    }

    let mut negatives: Vec<String> = Vec::new();
    for _ in 0..negative_count * 4 {
        if negatives.len() >= negative_count {
            break;
        }
        if let Some(text) = sample_negative_late(r, rng, SAMPLE_MAX_STAR_ITERS) {
            if !negatives.contains(&text) {
                negatives.push(text);
            }
        }
    }

    positives
        .into_iter()
        .map(|text| Candidate { text, provenance: Provenance::Structural, claimed_match: true })
        .chain(negatives.into_iter().map(|text| Candidate {
            text,
            provenance: Provenance::StructuralNegative,
            claimed_match: false,
        }))
        .collect()
}

// -------------------------------
// Source-specific generation
// -------------------------------

/// Suricata: join the rule's own `content:` field bytes, with a short
/// filler between consecutive fields.
/// A rule with fewer than two fields has no filler to vary, so it
/// converges on one candidate.
pub fn generate_from_content_fields(
    fields: &[ContentField],
    pattern_core: &Regex,
    rng: &mut StdRng,
    count: usize,
) -> Vec<Candidate> {
    if fields.is_empty() {
        return Vec::new();
    }
    let count = count.max(1);
    let mut texts: Vec<String> = Vec::new();
    for _ in 0..count * 3 {
        if texts.len() >= count {
            break;
        }
        let mut text = String::new();
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                if let Some(filler) = sample_positive(pattern_core, rng, 1) {
                    let take = filler.chars().take(3).collect::<String>();
                    text.push_str(&take);
                }
            }
            text.push_str(&String::from_utf8_lossy(&field.bytes));
        }
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    texts
        .into_iter()
        .map(|text| Candidate { text, provenance: Provenance::ContentDerived, claimed_match: true })
        .collect()
}

/// SpamAssassin: sample real messages from a local corpus directory that
/// match the pattern.
/// Returns empty if the directory does not exist or the pattern does not
/// parse.
///
/// Uses `prepare::core_regex`, not `parse_pcre_rule` directly: the same
/// `raw_pattern` was already accepted via `core_regex_tagged`, and some
/// accepted patterns only parse because of `core_regex`'s `^...$`
/// stripping; re-deriving with the same function avoids dropping those.
///
/// Under full-string matching, "match" means the entire file content
/// equals a match; most corpus files are not exact matches of a short
/// pattern, so this is expected to find little against a real corpus.
pub fn generate_from_corpus_dir(
    corpus_dir: &Path,
    raw_pattern: &str,
    sample_count: usize,
    rng: &mut StdRng,
) -> Vec<Candidate> {
    let Ok(r) = crate::data::prepare::core_regex(raw_pattern) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(corpus_dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    files.shuffle(rng);
    files
        .into_iter()
        .take(sample_count)
        .filter_map(|path| std::fs::read_to_string(&path).ok())
        .filter(|text| crate::data::prepare::matches_bounded(text, &r) == Some(true))
        .map(|text| Candidate { text, provenance: Provenance::RealCorpus, claimed_match: true })
        .collect()
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
    fn content_fields_assemble_into_up_to_count_candidates() {
        let fields = vec![
            ContentField {
                bytes: b"PDF-".to_vec(),
                nocase: false,
                distance: None,
                within: None,
                depth: Some(300),
                offset: None,
            },
            ContentField {
                bytes: b"Launch".to_vec(),
                nocase: false,
                distance: Some(0),
                within: None,
                depth: None,
                offset: None,
            },
        ];
        let core = Regex::star(Regex::lit('x'));
        let mut rng = rng();
        let candidates = generate_from_content_fields(&fields, &core, &mut rng, 5);
        assert!(!candidates.is_empty());
        assert!(candidates.len() <= 5);
        for candidate in &candidates {
            assert!(candidate.text.contains("PDF-"));
            assert!(candidate.text.contains("Launch"));
            assert_eq!(candidate.provenance, Provenance::ContentDerived);
        }
    }

    #[test]
    fn content_fields_with_no_gap_converge_on_one_candidate() {
        let fields = vec![ContentField {
            bytes: b"PDF-".to_vec(),
            nocase: false,
            distance: None,
            within: None,
            depth: None,
            offset: None,
        }];
        let core = Regex::star(Regex::lit('x'));
        let mut rng = rng();
        let candidates = generate_from_content_fields(&fields, &core, &mut rng, 5);
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn corpus_dir_missing_yields_no_candidates() {
        let mut rng = rng();
        let candidates = generate_from_corpus_dir(
            Path::new("/nonexistent/path/for/tests"),
            r"[a-c]+\d{2}",
            5,
            &mut rng,
        );
        assert!(candidates.is_empty());
    }

    #[test]
    fn corpus_dir_only_keeps_messages_that_actually_match() {
        let dir = std::env::temp_dir().join(format!("data-corpus-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("1.txt"), "ab12").unwrap();
        std::fs::write(dir.join("2.txt"), "this one has ab12 in it, plus more text").unwrap();

        let mut rng = rng();
        let candidates = generate_from_corpus_dir(&dir, r"[a-c]+\d{2}", 10, &mut rng);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].text, "ab12");

        let _ = std::fs::remove_dir_all(&dir);
    }
}