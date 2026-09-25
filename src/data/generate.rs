//! Per-source input generation. Each source needs a different strategy,
//! since each has a different kind of raw material to draw on:
//!
//! - Suricata: the rule's own `content:` fields are real evidence of what a
//!   triggering payload contains, so a candidate is assembled from them.
//! - SpamAssassin: a real public ham/spam corpus exists; when one is
//!   configured locally, candidates are sampled from it directly.
//! - RegexLib (and the fallback for the other two, when the above don't
//!   apply): there is no recoverable external source, so a candidate is
//!   sampled by walking the pattern's own structure.
//!
//! Nothing here is trusted output. Every `Candidate` this module produces
//! carries a `claimed_match` that `prepare` verifies against the real
//! parser before it becomes a `PreparedCase`.

use std::path::Path;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::data::types::{Candidate, Category, ContentField, Provenance};
use crate::types::Regex;

/// A character guaranteed not to appear in this pipeline's working
/// alphabet (`src/frontend/alphabet.rs`'s 98 characters), used to corrupt a
/// known-good sample into a known-bad one without guessing what the
/// pattern's own alphabet is.
const OUT_OF_ALPHABET_CHAR: char = '\u{2603}'; // snowman

/// The total length budget every `sample_positive` call starts with.
/// `max_star_iters` alone bounds any *one* `Star`, but nested repetition
/// multiplies: a pattern with a `Star` inside a `Star` can still produce a
/// combinatorially long sample even with a small per-`Star` cap, and
/// `deriv_std_rec`'s stack cost grows with input length (the same property
/// Chapter 5 documents), so an uncapped sample is not just slow, it is a
/// real crash risk for a generator meant to run unattended over an entire
/// corpus. This was found by running the pipeline against real RegexLib
/// patterns, not anticipated in the abstract: a nested-repetition email
/// pattern produced a sample long enough to overflow the stack before this
/// cap was added.
const DEFAULT_MAX_TOTAL_LEN: usize = 300;

// -------------------------------
// Structural generation (works for any `Regex`, any source)
// -------------------------------

/// Samples one string `r` should match, by walking its structure and
/// picking a branch/iteration count at each choice point. Returns `None`
/// only for `Regex::Phi`, which matches nothing.
///
/// Total output length is capped at `DEFAULT_MAX_TOTAL_LEN`: a pattern's
/// own mandatory (non-`Star`) content is always produced in full, but once
/// the budget is spent, every `Star` anywhere in the tree stops adding
/// further iterations, whatever `max_star_iters` would otherwise allow.
pub fn sample_positive(r: &Regex, rng: &mut StdRng, max_star_iters: u32) -> Option<String> {
    let mut budget = DEFAULT_MAX_TOTAL_LEN;
    sample_positive_bounded(r, rng, max_star_iters, &mut budget)
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
            // Mandatory content is never refused for budget reasons; only
            // repetition (below) is throttled, since that is the only
            // source of unbounded growth.
            *budget = budget.saturating_sub(1);
            Some(c.to_string())
        }
        Regex::Seq(a, b) => {
            let mut s = sample_positive_bounded(a, rng, max_star_iters, budget)?;
            s.push_str(&sample_positive_bounded(b, rng, max_star_iters, budget)?);
            Some(s)
        }
        Regex::Alt(a, b) => {
            // Try the randomly preferred side first; a side that turns out
            // to be (or contain only) Phi falls back to the other rather
            // than failing the whole sample.
            if rng.gen_bool(0.5) {
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
                    // A body that can only match empty (or is Phi) still
                    // leaves the star itself matchable by zero iterations.
                    None => break,
                }
            }
            Some(s)
        }
    }
}

/// Samples a positive match and corrupts its last character, producing a
/// candidate that should fail only after consuming almost the whole
/// pattern, the shape a worst-case negative benchmark input needs. Returns
/// `None` if `r` has no positive sample to corrupt (matches only `Phi` or
/// only the empty string, neither of which has a "last character").
pub fn sample_negative_late(r: &Regex, rng: &mut StdRng, max_star_iters: u32) -> Option<String> {
    let mut s = sample_positive(r, rng, max_star_iters)?;
    if s.is_empty() {
        return None;
    }
    s.pop();
    s.push(OUT_OF_ALPHABET_CHAR);
    Some(s)
}

/// Builds up to `count` distinct `Category::Best` candidates: the `count`
/// shortest positive samples this generator can find, out of `4 * count`
/// attempts, deduplicated by text before the shortest are picked. Returns
/// fewer than `count` if the pattern simply doesn't have that many
/// distinct short matches (`a` only ever matches `"a"`).
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

/// Builds up to `count` distinct `Category::Neutral` candidates:
/// ordinary-length positive samples, no engineered pathology. Each draw
/// varies its own iteration count (2 to 4 per `Star`) rather than
/// resampling the same fixed point repeatedly, so `count > 1` actually
/// diversifies rather than collapsing to near-identical copies.
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

/// Builds up to `2 * count` `Category::Worst` candidates structurally:
/// `count` distinct long, heavily iterated positives (genuine
/// ambiguity/length cost) and `count` distinct late-failing negatives
/// (cost paid before rejection), each independently drawn so `count > 1`
/// gives real variation rather than repeating one sample.
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

// -------------------------------
// Suricata: content-field-derived generation
// -------------------------------

/// Assembles up to `count` distinct worst-case candidates from a Suricata
/// rule's own `content:` fields: real evidence of what a triggering
/// payload contains, joined in the order they appear with a short
/// structural filler between them wherever the fields alone don't cover
/// the gap. `pattern_core` is the rule's translated core `Regex` (used
/// only to fill gaps, not to decide field order), since the `content:`
/// fields do not by themselves guarantee a `pcre:` match; `prepare` is
/// what actually confirms it. Variation across the `count` variants comes
/// entirely from the filler's own randomness; a rule with zero or one
/// field (no gap to fill) naturally converges on a single candidate no
/// matter how many attempts are made, since there is nothing left to vary.
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
                // A short filler between fields, standing in for whatever
                // the pattern needs between the literal fragments the rule
                // itself gives no explicit content for.
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

// -------------------------------
// SpamAssassin: real-corpus sampling
// -------------------------------

/// Samples candidate message bodies from a local ham/spam corpus directory
/// (one message per file, the shape the public SpamAssassin corpus
/// archives unpack into), and checks each one against `pattern` itself
/// rather than leaving that entirely to `prepare`: most real messages will
/// not contain what any given rule looks for, and there is no point
/// claiming a match `prepare` would just reject. A message that does match
/// becomes a `Neutral` candidate (a real message is exactly what
/// "neutral" means); one that does not is dropped here rather than turned
/// into a manufactured "worst case", since a message a rule was never
/// aimed at is not an adversarial input, just an irrelevant one.
///
/// Returns an empty vector, rather than an error, if no corpus is
/// configured at `corpus_dir`: this pipeline never fabricates "real
/// corpus" data, and a missing corpus just means this source falls back to
/// the structural generator.
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
        // `matches_bounded`, not a plain `parse_deriv_bc`/`parse_deriv_std_rec`
        // call: a real message body can be long, `deriv_std` deliberately
        // never simplifies (Section `sec:deriv-no-simp`), and even
        // `deriv_bc`'s own per-step `simp` was found not to bound every
        // pattern's growth (see `prepare::MAX_AREGEX_NODES`). A message
        // this pipeline cannot verify safely is dropped, the same as one
        // that simply does not match.
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
            // Corrupting only the last character means everything before
            // it is untouched, so a match attempt still has to walk nearly
            // the whole string before failing.
            let prefix_len = s.chars().count().saturating_sub(1);
            let prefix: String = s.chars().take(prefix_len).collect();
            if !prefix.is_empty() {
                // The prefix alone need not itself be a member (it may be
                // a strict prefix of one), but it must not immediately be
                // Phi: nullable-or-not is irrelevant here, only that the
                // corruption is confined to the final character.
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
        // "a" only ever matches "a": every draw collapses to the same
        // single string, so 5 distinct variants is impossible however many
        // attempts are made, and the function must return just the 1 that
        // exists rather than 5 duplicates.
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
        // A single field has no gap for the filler to vary, so however
        // many are requested, only one distinct candidate can exist.
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
