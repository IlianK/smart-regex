//! src/data/generate.rs
//!
//! Per-source input generation. Each source draws on different raw
//! material:
//!
//! - Snort: the rule's own `content:` fields are real evidence of what a
//!   triggering payload contains, so a candidate is assembled from them.
//! - SpamAssassin: a real public ham/spam corpus exists; when one is
//!   configured locally, candidates are sampled from it directly.
//! - RegexLib (and the fallback for the other two, when the above don't
//!   apply): there is no recoverable external source, so a candidate is
//!   sampled by walking the pattern's own structure.
//!
//! Every `Candidate` carries a `claimed_match` that `prepare` verifies
//! against the real parser before it becomes a `PreparedCase`.

use std::path::Path;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::data::types::{Candidate, Category, ContentField, Provenance};
use crate::types::Regex;

const OUT_OF_ALPHABET_CHAR: char = '\u{2603}';
const DEFAULT_MAX_TOTAL_LEN: usize = 300;

// SAMPLING

pub fn sample_positive(r: &Regex, rng: &mut StdRng, max_star_iters: u32) -> Option<String> {
    let mut budget = DEFAULT_MAX_TOTAL_LEN;
    sample_positive_bounded(r, rng, max_star_iters, &mut budget)
}

/// Number of `Alt` leaves reachable under `r`, treating it as a flat list
/// of alternatives when it is a chain of `Alt` nodes. Used to weight
/// sampling so a left-folded chain is chosen from uniformly; see the
/// comment in `sample_positive_bounded`'s `Alt` case.
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
            // `alt_of_chars` (frontend/translate.rs) builds a character
            // class as a deeply left-nested `Alt` chain: a flat 50/50
            // choice would pick the last-inserted character about half
            // the time, so for a class of size N most of its early-sorted
            // members would never be sampled. Weighting by leaf count
            // makes every leaf equally likely regardless of tree shape.
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

// BEST

pub fn generate_best(r: &Regex, rng: &mut StdRng, count: usize) -> Vec<Candidate> {
    let count = count.max(1);
    let mut samples: Vec<String> = Vec::new();
    for _ in 0..count * 4 {
        if let Some(s) = sample_positive(r, rng, 1) {
            if !samples.contains(&s) {
                samples.push(s);
            }
        }
    }
    samples.sort_by_key(|s| s.len());
    samples.truncate(count);
    samples
        .into_iter()
        .map(|text| Candidate {
            text,
            category: Category::Best,
            provenance: Provenance::Structural,
            claimed_match: true,
        })
        .collect()
}

// NEUTRAL

pub fn generate_neutral(r: &Regex, rng: &mut StdRng, count: usize) -> Vec<Candidate> {
    let count = count.max(1);
    let mut samples: Vec<String> = Vec::new();
    for _ in 0..count * 3 {
        if samples.len() >= count {
            break;
        }
        let iters = rng.gen_range(2..=4);
        if let Some(s) = sample_positive(r, rng, iters) {
            if !samples.contains(&s) {
                samples.push(s);
            }
        }
    }
    samples
        .into_iter()
        .map(|text| Candidate {
            text,
            category: Category::Neutral,
            provenance: Provenance::Structural,
            claimed_match: true,
        })
        .collect()
}

// WORST

pub fn generate_worst_structural(
    r: &Regex,
    rng: &mut StdRng,
    max_star_iters: u32,
    count: usize,
) -> Vec<Candidate> {
    let count = count.max(1);

    let mut positives: Vec<String> = Vec::new();
    for _ in 0..count * 3 {
        if positives.len() >= count {
            break;
        }
        if let Some(text) = sample_positive(r, rng, max_star_iters) {
            if !positives.contains(&text) {
                positives.push(text);
            }
        }
    }

    let mut negatives: Vec<String> = Vec::new();
    for _ in 0..count * 3 {
        if negatives.len() >= count {
            break;
        }
        if let Some(text) = sample_negative_late(r, rng, max_star_iters) {
            if !negatives.contains(&text) {
                negatives.push(text);
            }
        }
    }

    positives
        .into_iter()
        .map(|text| Candidate {
            text,
            category: Category::Worst,
            provenance: Provenance::Structural,
            claimed_match: true,
        })
        .chain(negatives.into_iter().map(|text| Candidate {
            text,
            category: Category::Worst,
            provenance: Provenance::StructuralNegative,
            claimed_match: false,
        }))
        .collect()
}

// SOURCE-SPECIFIC

/// Suricata: join the rule's own `content:` field bytes, inserting a short
/// filler between consecutive fields. A rule with fewer than two fields has
/// no filler to vary, so it converges on one candidate.
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
        .map(|text| Candidate {
            text,
            category: Category::Worst,
            provenance: Provenance::ContentDerived,
            claimed_match: true,
        })
        .collect()
}

/// SpamAssassin: sample real messages from a local corpus directory that
/// actually match the pattern. Returns empty if the directory does not
/// exist or the pattern does not parse.
pub fn generate_from_corpus_dir(
    corpus_dir: &Path,
    raw_pattern: &str,
    sample_count: usize,
    rng: &mut StdRng,
) -> Vec<Candidate> {
    let Ok(r) = crate::frontend::parse_pcre_rule(raw_pattern) else {
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
        .map(|text| Candidate {
            text,
            category: Category::Neutral,
            provenance: Provenance::RealCorpus,
            claimed_match: true,
        })
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
        // Regression: `alt_of_chars` builds a 36-member class as a
        // left-folded `Alt` chain. A flat 50/50 choice used to make the
        // last-sorted characters dominate almost completely, with the
        // first ~20 members never sampled at all.
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
    fn generate_best_prefers_shorter_samples() {
        let r = core(r"a+");
        let mut rng = rng();
        let best = generate_best(&r, &mut rng, 10);
        assert!(!best.is_empty(), "should generate at least one");
        assert!(
            best[0].text.len() <= 3,
            "expected the shortest sample first, got {:?}",
            best[0].text
        );
        assert!(best.iter().all(|c| c.category == Category::Best));
    }

    #[test]
    fn generate_best_deduplicates_and_caps_at_count() {
        let r = core(r"a");
        let mut rng = rng();
        let best = generate_best(&r, &mut rng, 5);
        assert_eq!(best.len(), 1);
        assert_eq!(best[0].text, "a");
    }

    #[test]
    fn generate_neutral_returns_up_to_count_distinct_variants() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        let neutral = generate_neutral(&r, &mut rng, 5);
        assert!(!neutral.is_empty());
        assert!(neutral.len() <= 5);
        let distinct: std::collections::HashSet<_> = neutral.iter().map(|c| &c.text).collect();
        assert_eq!(distinct.len(), neutral.len(), "no duplicate texts");
        assert!(neutral.iter().all(|c| c.category == Category::Neutral));
    }

    #[test]
    fn generate_worst_structural_returns_up_to_count_of_each_kind() {
        let r = core(r"[a-c]+\d{2}");
        let mut rng = rng();
        let worst = generate_worst_structural(&r, &mut rng, 6, 5);
        let positives = worst.iter().filter(|c| c.provenance == Provenance::Structural).count();
        let negatives = worst.iter().filter(|c| c.provenance == Provenance::StructuralNegative).count();
        assert!(positives >= 1 && positives <= 5);
        assert!(negatives >= 1 && negatives <= 5);
        assert!(worst.iter().all(|c| c.category == Category::Worst));
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
        std::fs::write(dir.join("1.txt"), "this one has ab12 in it").unwrap();
        std::fs::write(dir.join("2.txt"), "this one has nothing relevant").unwrap();

        let mut rng = rng();
        let candidates = generate_from_corpus_dir(&dir, r"[a-c]+\d{2}", 10, &mut rng);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].text.contains("ab12"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}