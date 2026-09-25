
use std::path::Path;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::data::types::{Candidate, Category, ContentField, Provenance};
use crate::types::Regex;


const OUT_OF_ALPHABET_CHAR: char = '\u{2603}'; 
const DEFAULT_MAX_TOTAL_LEN: usize = 300;

// -------------------------------
// Structural generation (works for any `Regex`, any source)
// -------------------------------
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
            *budget = budget.saturating_sub(1);
            Some(c.to_string())
        }
        Regex::Seq(a, b) => {
            let mut s = sample_positive_bounded(a, rng, max_star_iters, budget)?;
            s.push_str(&sample_positive_bounded(b, rng, max_star_iters, budget)?);
            Some(s)
        }
        Regex::Alt(a, b) => {
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

pub fn generate_best(r: &Regex, rng: &mut StdRng, attempts: u32) -> Option<Candidate> {
    let mut shortest: Option<String> = None;
    for _ in 0..attempts.max(1) {
        if let Some(s) = sample_positive(r, rng, 1) {
            if shortest.as_ref().is_none_or(|cur| s.len() < cur.len()) {
                shortest = Some(s);
            }
        }
    }
    shortest.map(|text| Candidate {
        text,
        category: Category::Best,
        provenance: Provenance::Structural,
        claimed_match: true,
    })
}

pub fn generate_neutral(r: &Regex, rng: &mut StdRng) -> Option<Candidate> {
    sample_positive(r, rng, 3).map(|text| Candidate {
        text,
        category: Category::Neutral,
        provenance: Provenance::Structural,
        claimed_match: true,
    })
}
pub fn generate_worst_structural(r: &Regex, rng: &mut StdRng, max_star_iters: u32) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(text) = sample_positive(r, rng, max_star_iters) {
        out.push(Candidate {
            text,
            category: Category::Worst,
            provenance: Provenance::Structural,
            claimed_match: true,
        });
    }
    if let Some(text) = sample_negative_late(r, rng, max_star_iters) {
        out.push(Candidate {
            text,
            category: Category::Worst,
            provenance: Provenance::StructuralNegative,
            claimed_match: false,
        });
    }
    out
}

// -------------------------------
// Suricata: content-field-derived generation
// -------------------------------

pub fn generate_from_content_fields(
    fields: &[ContentField],
    pattern_core: &Regex,
    rng: &mut StdRng,
) -> Option<Candidate> {
    if fields.is_empty() {
        return None;
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
    Some(Candidate {
        text,
        category: Category::Worst,
        provenance: Provenance::ContentDerived,
        claimed_match: true,
    })
}

// -------------------------------
// SpamAssassin: real-corpus sampling
// -------------------------------

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
        let best = generate_best(&r, &mut rng, 10).expect("should generate");
        assert!(best.text.len() <= 3, "expected a short sample, got {:?}", best.text);
        assert_eq!(best.category, Category::Best);
    }

    #[test]
    fn content_fields_assemble_into_one_candidate() {
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
        let candidate = generate_from_content_fields(&fields, &core, &mut rng).expect("should assemble");
        assert!(candidate.text.contains("PDF-"));
        assert!(candidate.text.contains("Launch"));
        assert_eq!(candidate.provenance, Provenance::ContentDerived);
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
