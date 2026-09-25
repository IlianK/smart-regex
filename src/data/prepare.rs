use crate::data::types::{Candidate, Category, PreparedCase, SourceKind};
use crate::frontend::{case_fold, parse_ext_pattern, parse_pcre_rule, strip_pcre_delimiters, translate, ExtPat};
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::internalize::internalize;
use crate::parsers::deriv_bc::nullable::{is_phi, nullable_bc};
use crate::parsers::deriv_bc::simplify::simp;
use crate::types::{ARegex, Regex};


const MAX_AREGEX_NODES: usize = 100_000;

fn aregex_node_count(r: &ARegex) -> usize {
    let mut stack = vec![r];
    let mut count = 0usize;
    while let Some(node) = stack.pop() {
        count += 1;
        if count > MAX_AREGEX_NODES {
            return count;
        }
        match node {
            ARegex::Phi | ARegex::Eps(_) | ARegex::Lit(_, _) => {}
            ARegex::Alt(_, a, b) | ARegex::Seq(_, a, b) => {
                stack.push(a);
                stack.push(b);
            }
            ARegex::Star(_, a) => stack.push(a),
        }
    }
    count
}

enum SteppedOutcome {
    DiedAt(usize),
    Finished { nullable: bool },
    TooExpensive,
}

fn step_bc_bounded(input: &str, r: &Regex) -> SteppedOutcome {
    let mut ri = internalize(r);
    if aregex_node_count(&ri) > MAX_AREGEX_NODES {
        return SteppedOutcome::TooExpensive;
    }
    let mut consumed = 0usize;
    for c in input.chars() {
        ri = simp(deriv_bc(ri, c));
        consumed += 1;
        if aregex_node_count(&ri) > MAX_AREGEX_NODES {
            return SteppedOutcome::TooExpensive;
        }
        if is_phi(&ri) {
            return SteppedOutcome::DiedAt(consumed);
        }
    }
    SteppedOutcome::Finished { nullable: nullable_bc(&ri) }
}

pub(crate) fn matches_bounded(input: &str, r: &Regex) -> Option<bool> {
    match step_bc_bounded(input, r) {
        SteppedOutcome::DiedAt(_) => Some(false),
        SteppedOutcome::Finished { nullable } => Some(nullable),
        SteppedOutcome::TooExpensive => None,
    }
}


pub fn failed_after_chars(input: &str, r: &Regex) -> Option<usize> {
    match step_bc_bounded(input, r) {
        SteppedOutcome::DiedAt(consumed) => Some(consumed),
        SteppedOutcome::Finished { .. } => Some(input.chars().count()),
        SteppedOutcome::TooExpensive => None,
    }
}

const LATE_FAILURE_THRESHOLD: f64 = 2.0 / 3.0;


pub fn core_regex(raw_pattern: &str) -> Result<Regex, String> {
    let (body, case_insensitive) = strip_pcre_delimiters(raw_pattern);
    let ep = parse_ext_pattern(body)?;
    let ep = if case_insensitive { case_fold(&ep) } else { ep };

    let estimated_depth = estimated_translated_depth(&ep);
    if estimated_depth > MAX_CORE_DEPTH {
        return Err(format!(
            "pattern is estimated to translate to a nesting depth of at least {estimated_depth}, \
             exceeding this pipeline's cap of {MAX_CORE_DEPTH}; likely a character class whose \
             range spans far more code points than intended (for example `[\\--\u{2013}]`, parsed \
             as every code point from `-` to the en dash, found by running this pipeline against \
             a real RegexLib pattern), rather than a pattern that is simply large. Rejected \
             before translation, since translating a pattern this deep is itself the risk."
        ));
    }

    let r = translate(&ep)?;
    let nodes = node_count(&r);
    if nodes > MAX_CORE_NODES {
        return Err(format!(
            "translated pattern has {nodes} nodes, exceeding this pipeline's cap of {MAX_CORE_NODES}"
        ));
    }
    Ok(r)
}


fn estimated_translated_depth(ep: &ExtPat) -> usize {
    match ep {
        ExtPat::Empty | ExtPat::Eps | ExtPat::Never | ExtPat::Char(_) | ExtPat::Escape(_) => 1,
        ExtPat::Carat | ExtPat::Dollar => 1,
        ExtPat::Dot => 98, // the full working alphabet, alt_of_chars(alphabet())'s Alt chain
        ExtPat::Any(cs) | ExtPat::NoneOf(cs) => cs.len().max(1),
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => estimated_translated_depth(inner),
        ExtPat::Opt(inner, _) => 1 + estimated_translated_depth(inner),
        ExtPat::Plus(inner, _) => 1 + 2 * estimated_translated_depth(inner),
        ExtPat::Star(inner, _) => 1 + estimated_translated_depth(inner),
        ExtPat::Or(parts) => 1 + parts.iter().map(estimated_translated_depth).max().unwrap_or(0),
        ExtPat::Concat(parts) => parts.iter().map(estimated_translated_depth).sum::<usize>() + parts.len(),
        ExtPat::WordBoundary(_) => 1,
        ExtPat::Bound(inner, lo, hi, _) => {
            let copies = (*hi).unwrap_or(*lo).max(*lo).max(1) as usize;
            copies.saturating_add(estimated_translated_depth(inner))
        }
    }
}


fn node_count(r: &Regex) -> usize {
    let mut stack = vec![r];
    let mut count = 0usize;
    while let Some(node) = stack.pop() {
        count += 1;
        if count > MAX_CORE_NODES {
            return count;
        }
        match node {
            Regex::Phi | Regex::Eps | Regex::Lit(_) => {}
            Regex::Seq(a, b) | Regex::Alt(a, b) => {
                stack.push(a);
                stack.push(b);
            }
            Regex::Star(a) => stack.push(a),
        }
    }
    count
}


const MAX_CORE_DEPTH: usize = 2_000;
const MAX_CORE_NODES: usize = 20_000;

pub fn verify_candidate(
    source: SourceKind,
    raw_pattern: &str,
    candidate: Candidate,
) -> Option<PreparedCase> {

    let r_search = parse_pcre_rule(raw_pattern).ok()?;
    let verified_match = matches_bounded(&candidate.text, &r_search)?;
    if verified_match != candidate.claimed_match {
        return None;
    }

    let failed_after = if !verified_match {
        let r_core = core_regex(raw_pattern).ok()?;
        let consumed = failed_after_chars(&candidate.text, &r_core)?;
        let total = candidate.text.chars().count().max(1);
        if candidate.category == Category::Worst
            && (consumed as f64) < LATE_FAILURE_THRESHOLD * (total as f64)
        {
            return None;
        }
        Some(consumed)
    } else {
        None
    };

    Some(PreparedCase {
        source,
        pattern: raw_pattern.to_string(),
        input: candidate.text,
        category: candidate.category,
        provenance: candidate.provenance,
        verified_match,
        failed_after_chars: failed_after,
    })
}

/// Verifies every candidate in `candidates`, 
pub fn verify_all(
    source: SourceKind,
    raw_pattern: &str,
    candidates: Vec<Candidate>,
) -> Vec<PreparedCase> {
    candidates
        .into_iter()
        .filter_map(|c| verify_candidate(source, raw_pattern, c))
        .collect()
}


/// Appends `cases` to `path` as JSON lines, creating the file 
pub fn write_prepared_cases(
    path: &std::path::Path,
    cases: &[PreparedCase],
) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    for case in cases {
        let line = serde_json::to_string(case)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(file, "{line}")?;
    }
    Ok(())
}

/// Reads every prepared case out of a JSON-lines 
pub fn read_prepared_cases(path: &std::path::Path) -> std::io::Result<Vec<PreparedCase>> {
    let text = std::fs::read_to_string(path)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })
        .collect()
}

// -------------------------------
// Tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::generate;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn core(pattern: &str) -> Regex {
        let ep = crate::frontend::parse_ext_pattern(pattern).expect("should parse");
        translate(&ep).expect("should translate")
    }

    #[test]
    fn failed_after_chars_is_full_length_on_a_real_match() {
        let r = core(r"[a-c]+\d{2}");
        let n = failed_after_chars("ab12", &r).expect("should be verifiable");
        assert_eq!(n, 4);
    }

    #[test]
    fn failed_after_chars_stops_at_the_dead_position() {
        // "a" then an unmatchable character: dies at position 2, well
        // before the full (much longer) input is consumed.
        let r = core(r"a\d");
        let n = failed_after_chars("aXXXXXXXX", &r).expect("should be verifiable");
        assert_eq!(n, 2);
    }

    #[test]
    fn failed_after_chars_gives_up_past_the_node_cap() {
        let r = core(r#"</?(\w+)(\s*\w*\s*=\s*("[^"]*"|'[^']'|[^>]*))*|/?>"#);
        let input = "<wxxzuzzwyyz=}| \n \r\r\r=\t\t \r\t\"}~~|~}\"\n\t\rzzy    \n\n=\r'~'\t \r\t =     \"~~}}~}\"y=\r \n\"~z}}\"\nz \r =\r\t~~~";
        assert!(failed_after_chars(input, &r).is_none());
    }

    #[test]
    fn a_genuine_positive_candidate_survives_verification() {
        let pattern = r"[a-c]+\d{2}";
        let candidate = Candidate {
            text: "ab12".to_string(),
            category: Category::Neutral,
            provenance: crate::data::types::Provenance::Structural,
            claimed_match: true,
        };
        let prepared = verify_candidate(SourceKind::RegexLib, pattern, candidate)
            .expect("a genuine match should survive verification");
        assert!(prepared.verified_match);
    }

    #[test]
    fn a_false_claim_is_rejected() {
        let pattern = r"[a-c]+\d{2}";
        let candidate = Candidate {
            text: "zzz".to_string(), // does not match
            category: Category::Neutral,
            provenance: crate::data::types::Provenance::Structural,
            claimed_match: true, // wrongly claims it does
        };
        assert!(verify_candidate(SourceKind::RegexLib, pattern, candidate).is_none());
    }

    #[test]
    fn a_candidate_too_expensive_to_verify_is_dropped_not_hung() {
        let pattern = r#"</?(\w+)(\s*\w*\s*=\s*("[^"]*"|'[^']'|[^>]*))*|/?>"#;
        let candidate = Candidate {
            text: "<wxxzuzzwyyz=}| \n \r\r\r=\t\t \r\t\"}~~|~}\"\n\t\rzzy    \n\n=\r'~'\t \r\t =     \"~~}}~}\"y=\r \n\"~z}}\"\nz \r =\r\t~~~".to_string(),
            category: Category::Worst,
            provenance: crate::data::types::Provenance::Structural,
            claimed_match: true,
        };
        assert!(verify_candidate(SourceKind::RegexLib, pattern, candidate).is_none());
    }

    #[test]
    fn a_trivially_failing_worst_case_candidate_is_rejected() {
        let pattern = r"[a-c]+\d{2}";
        let candidate = Candidate {
            text: "zzzzzzzzzz".to_string(),
            category: Category::Worst,
            provenance: crate::data::types::Provenance::Structural,
            claimed_match: false,
        };
        assert!(verify_candidate(SourceKind::RegexLib, pattern, candidate).is_none());
    }

    #[test]
    fn a_late_failing_worst_case_candidate_is_accepted() {
        let pattern = r"[a-c]+\d{2}";
        let mut rng = StdRng::seed_from_u64(7);
        let r = core(pattern);
        let text = generate::sample_negative_late(&r, &mut rng, 4).expect("should generate");
        let candidate = Candidate {
            text,
            category: Category::Worst,
            provenance: crate::data::types::Provenance::StructuralNegative,
            claimed_match: false,
        };
        let prepared = verify_candidate(SourceKind::RegexLib, pattern, candidate)
            .expect("a genuinely late failure should survive verification");
        assert!(!prepared.verified_match);
        assert!(prepared.failed_after_chars.is_some());
    }

    #[test]
    fn prepared_cases_round_trip_through_jsonl() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cases.jsonl");
        let _ = std::fs::remove_file(&path);

        let case = PreparedCase {
            source: SourceKind::SpamAssassin,
            pattern: r"/x.{0,2}a.{0,2}n.{0,2}a.{0,2}x/i".to_string(),
            input: "buy xanax now".to_string(),
            category: Category::Neutral,
            provenance: crate::data::types::Provenance::Structural,
            verified_match: true,
            failed_after_chars: None,
        };
        write_prepared_cases(&path, std::slice::from_ref(&case)).unwrap();
        write_prepared_cases(&path, std::slice::from_ref(&case)).unwrap(); 

        let read_back = read_prepared_cases(&path).unwrap();
        assert_eq!(read_back.len(), 2);
        assert_eq!(read_back[0].pattern, case.pattern);
        assert_eq!(read_back[0].input, case.input);
        assert_eq!(read_back[0].verified_match, case.verified_match);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pathologically_wide_character_class_range_is_rejected() {
        let pattern = r"[0-9\--– ]{10}";
        let err = core_regex(pattern).expect_err("should be rejected, not translated");
        assert!(err.contains("depth"), "error should explain the depth cap: {err}");
    }

    #[test]
    fn ordinary_real_patterns_with_several_classes_are_not_rejected() {
        assert!(core_regex(r"^[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+)*@(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?\.)+(?:[a-zA-Z]{2}|aero|asia|biz|cat|com|coop|edu|gov|info|int|jobs|mil|mobi|museum|name|net|org|pro|tel|travel)$").is_ok());
        assert!(core_regex(r"^([\+][0-9]{1,3}([ \.\-])?)?([\(]{1}[0-9]{3}[\)])?([0-9A-Z \.\-]{1,32})((x|ext|extension)?[0-9]{1,4}?)$").is_ok());
    }

    #[test]
    fn an_ordinary_pattern_is_well_under_the_node_cap() {
        let r = core_regex(r"[a-c]+\d{2}").expect("should translate");
        assert!(node_count(&r) < 100);
    }
}
