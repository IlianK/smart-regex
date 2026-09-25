//! Verification and categorization: the mandatory step every candidate from
//! `generate` passes through before it is trusted. Nothing produced by
//! `generate` is written out as a `PreparedCase` without first being
//! checked against the real parser it claims an outcome for.
//!
//! For a `Category::Worst` negative specifically, verification also checks
//! that the rejection is late: a candidate that fails on the first
//! character is not a worst case, whatever its provenance claimed.

use crate::data::types::{Candidate, Category, PreparedCase, SourceKind};
use crate::frontend::{case_fold, parse_ext_pattern, parse_pcre_rule, strip_pcre_delimiters, translate, ExtPat};
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::internalize::internalize;
use crate::parsers::deriv_bc::nullable::{is_phi, nullable_bc};
use crate::parsers::deriv_bc::simplify::simp;
use crate::types::{ARegex, Regex};

/// A running cap on the bit-coded annotated expression's own node count,
/// checked after every character while stepping it through an input (see
/// `step_bc_bounded`). `simp` simplifies at every step, but simplifying is
/// not the same as bounding: a real RegexLib HTML-tag pattern
/// (`</?(\w+)(\s*\w*\s*=\s*("[^"]*"|'[^']'|[^>]*))*|/?>`, its `Star`
/// wrapping three overlapping alternatives) was found, by running this
/// pipeline's own generated candidates through it, to grow past a million
/// nodes within about 50 characters and keep growing, despite `simp`
/// running after every derivative. An ordinary corpus pattern's annotated
/// expression stays in the low thousands of nodes for an input this
/// pipeline would ever generate; 100,000 is generous for anything genuine
/// and rejects this kind of blowup within the first few dozen characters,
/// long before it threatens memory or the stack.
const MAX_AREGEX_NODES: usize = 100_000;

/// Counts every node in an annotated expression, iteratively (see `Regex`'s
/// own `node_count` for why: a recursive version would be vulnerable to
/// exactly the blowup this cap exists to catch), stopping as soon as the
/// cap is passed rather than finishing a count already known to be too
/// large.
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

/// Outcome of stepping a bit-coded annotated expression through an input
/// one character at a time, guarded by `MAX_AREGEX_NODES`.
enum SteppedOutcome {
    /// Went structurally `Phi` after consuming this many characters
    /// (always less than the input's length).
    DiedAt(usize),
    /// Consumed the whole input without dying; `nullable` says whether the
    /// final state accepts, i.e. whether the input matched.
    Finished { nullable: bool },
    /// The annotated expression exceeded `MAX_AREGEX_NODES` at some point.
    /// Too expensive to keep stepping safely; the caller should treat
    /// whatever it was trying to verify as unverifiable, not as a verdict.
    TooExpensive,
}

/// Steps `r`'s bit-coded annotated form (`internalize`, then `deriv_bc` +
/// `simp` per character) through `input`, stopping early either at the
/// first structurally `Phi` state (further derivation cannot change that)
/// or once the expression's own size passes `MAX_AREGEX_NODES`. Shared by
/// `verify_candidate`'s match check and `failed_after_chars`, so both go
/// through the same safety net rather than trusting `deriv_bc` + `simp` to
/// always stay small, which the pattern in `MAX_AREGEX_NODES`'s own doc
/// comment disproved.
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

/// Whether `input` matches `r`, checked through `step_bc_bounded` rather
/// than a plain `parse_deriv_bc` call, so a candidate this pipeline was
/// never able to verify safely (see `MAX_AREGEX_NODES`) comes back `None`
/// instead of hanging or exhausting memory. Shared by `verify_candidate`
/// and `generate::generate_from_corpus_dir`, since a real corpus message
/// can be at least as long as anything this pipeline itself generates.
pub(crate) fn matches_bounded(input: &str, r: &Regex) -> Option<bool> {
    match step_bc_bounded(input, r) {
        SteppedOutcome::DiedAt(_) => Some(false),
        SteppedOutcome::Finished { nullable } => Some(nullable),
        SteppedOutcome::TooExpensive => None,
    }
}

/// How far into `input` the parser gets before the match becomes
/// impossible: the number of characters consumed before the bit-coded
/// construction's annotated expression, kept simplified at every step,
/// becomes structurally `Phi`. `Some(input.len())` if the match never
/// becomes impossible (including if it succeeds); `None` if verifying this
/// safely would exceed `MAX_AREGEX_NODES`, in which case the caller should
/// drop the candidate rather than trust a partial measurement.
///
/// `r` should be the pattern's own core expression, not a search-padded
/// one: an unanchored search wraps the pattern in `Sigma*`, which stays
/// alive on any in-alphabet character indefinitely (it is always willing
/// to treat one more character as filler), so a padded expression almost
/// never goes structurally `Phi` before the input ends, whether or not the
/// core inside it ever had a chance. Measuring against the core is what
/// makes "how much genuine work did the match attempt do" answerable at
/// all for a search pattern.
///
/// This does not depend on `deriv_std`'s own no-simplification choice
/// (Section `sec:deriv-no-simp` in the thesis): `deriv_bc` always
/// simplifies, which is exactly what makes "has this gone completely dead
/// yet" a question this check can answer precisely rather than
/// approximately.
pub fn failed_after_chars(input: &str, r: &Regex) -> Option<usize> {
    match step_bc_bounded(input, r) {
        SteppedOutcome::DiedAt(consumed) => Some(consumed),
        SteppedOutcome::Finished { .. } => Some(input.chars().count()),
        SteppedOutcome::TooExpensive => None,
    }
}

/// The minimum fraction of the input a worst-case negative must consume
/// before failing for the rejection to count as "late" rather than
/// trivial. `2/3` was chosen so a corruption of only the final character
/// (this pipeline's own negative generator) always qualifies, while a
/// same-cost-as-immediate rejection never does.
const LATE_FAILURE_THRESHOLD: f64 = 2.0 / 3.0;

/// The pattern's own core `Regex` (translated, not search-padded), after
/// stripping any `/PATTERN/FLAGS` wrapper and applying case-folding for an
/// `i` flag: the same lowering `parse_pcre_rule` performs internally,
/// stopped one step short of adding the search padding. All three
/// extracted sources hand their raw pattern text through here identically;
/// `strip_pcre_delimiters` passes a bare, unwrapped pattern (RegexLib's
/// own shape) through unchanged rather than requiring the wrapper.
///
/// `generate` samples from this core; `verify_candidate` checks the
/// padded form (via `parse_pcre_rule` directly) but measures lateness
/// against this one, since a search-padded expression's own liveness is
/// not informative (see `failed_after_chars`).
pub fn core_regex(raw_pattern: &str) -> Result<Regex, String> {
    let (body, case_insensitive) = strip_pcre_delimiters(raw_pattern);
    let ep = parse_ext_pattern(body)?;
    let ep = if case_insensitive { case_fold(&ep) } else { ep };

    // Estimated *before* translating, not after: `translate` itself can
    // already be unsafe to call on a pattern whose character class range
    // is pathologically wide, since desugaring a bound clones the class's
    // whole expansion once per repetition (`Regex`'s own derived `Clone`
    // recurses over the tree it is copying), and a wide-enough class can
    // overflow the stack inside that clone before this function ever gets
    // a `Regex` value back to inspect. `ExtPat` itself is still safe to
    // walk recursively at this point: it has not yet been expanded by
    // `translate`, so its size is proportional to the source pattern
    // string, not to what a bound will multiply it into.
    //
    // This estimates *depth*, not node count: a `Seq`/`Concat` chain of
    // several separate, ordinary-sized character classes has a node count
    // in the low thousands but a shallow depth, since each class is a
    // sibling, not nested inside the last, so it is not a stack-overflow
    // risk despite a superficially large size. A single character class
    // with a pathologically wide range, on the other hand, is one deeply
    // left-nested `Alt` chain (`alt_of_chars`'s fold), so its depth *is*
    // its size. Depth is what recursive traversal (`nullable`, `deriv`,
    // `Regex`'s own derived `Clone`) actually pays for in stack, so it is
    // the right thing to bound, not total size, which an early version of
    // this check used and which rejected several perfectly ordinary real
    // corpus patterns (an email address pattern, a phone number pattern)
    // for having several unremarkable classes rather than one huge one.
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

/// An upper bound on the nesting depth `translate(ep)` would produce,
/// computed over `ExtPat` directly so a pathological pattern can be
/// rejected before `translate` is ever called on it (see `core_regex` for
/// why depth, not total size, is the right thing to bound). Siblings
/// (`Seq`/`Concat`'s two parts, `Bound`'s unrolled copies) add depth;
/// alternatives (`Alt`/`Or`'s branches) take the deeper of the two, since
/// only one is ever on the path to any given node.
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

/// Counts every node in `r`, `Regex`'s own structure walked directly
/// rather than estimated: the true size a character class with an
/// unexpectedly wide range expands to, and what a bounded repetition
/// multiplies it by.
///
/// Deliberately iterative, with an explicit heap-allocated stack, and not
/// the natural recursive walk: this function exists specifically to guard
/// against a pathologically large, lopsided tree, so it cannot itself use
/// the call stack to do the counting, or it would be vulnerable to
/// precisely what it is checking for. This was not a theoretical
/// precaution: a first, recursive version of this function was what
/// actually overflowed the stack while chasing the crash this cap exists
/// to prevent. It also exits as soon as the cap is passed, rather than
/// finish counting a tree already known to be too large.
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

/// A pattern estimated to nest deeper than this is rejected before
/// `translate` is ever called on it (see `core_regex`, and
/// `estimated_translated_depth`'s own doc comment for why depth rather
/// than total size). `nullable`, `deriv`, `parse_deriv_std_rec`'s own
/// construction, and `Regex`'s derived `Clone` (this thesis's own
/// recursive implementations, not rewritten to be iterative) cost stack
/// proportional to depth, and a tree this deep is a real, observed risk:
/// the pattern that motivated this cap (`[\--–]`, a character class range
/// spanning roughly 8000 code points) overflowed the stack inside
/// `Regex::clone` during a bound's desugaring, before this pipeline had a
/// `Regex` value to inspect at all. 2000 is comfortably below that, and
/// comfortably above any depth an ordinary corpus pattern's nesting
/// reaches in practice.
const MAX_CORE_DEPTH: usize = 2_000;

/// A translated pattern with more nodes than this is rejected outright,
/// as a second, coarser check after the depth estimate above: total size
/// still matters for a pipeline meant to process an entire corpus, even
/// for a tree too wide rather than too deep to threaten the stack.
/// `node_count` is iterative, so this check itself is safe regardless of
/// shape. A corpus pattern's ordinary cost is in the low hundreds of
/// nodes even with several character classes (Section `sec:frontend-cost`
/// of the thesis: a `\w` class alone is 125), so this is generous for
/// real patterns and only bites the pathological ones.
const MAX_CORE_NODES: usize = 20_000;

/// Verifies one candidate against the real parser and turns it into a
/// `PreparedCase`, or returns `None` if it does not hold up: either the
/// claimed outcome was wrong, or (for a worst-case negative) the failure
/// was not late enough to be a genuine worst case.
pub fn verify_candidate(
    source: SourceKind,
    raw_pattern: &str,
    candidate: Candidate,
) -> Option<PreparedCase> {
    // The same search-padded expression a benchmark actually runs (wrapper
    // stripped, case-folded, Sigma*-padded), so the match/no-match verdict
    // checks the exact thing that gets measured.
    let r_search = parse_pcre_rule(raw_pattern).ok()?;

    // `matches_bounded`, not a plain `parse_deriv_bc`/`parse_deriv_std_rec`
    // call: `parse_deriv_std_rec` is `deriv_std`'s own recursive parser,
    // which deliberately never simplifies (Section `sec:deriv-no-simp`),
    // and even `deriv_bc`'s per-step `simp` was found, by running this
    // pipeline's own generated candidates through a real RegexLib
    // HTML-tag pattern, not to bound every pattern's growth; that
    // candidate's annotated expression passed a million nodes within about
    // fifty characters. `matches_bounded` is the same bit-coded stepping,
    // with a hard size cap that gives up rather than let this pipeline
    // hang or exhaust memory verifying a candidate against a pattern this
    // pathological.
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

/// Verifies every candidate in `candidates`, keeping only the ones that
/// hold up. Order is preserved among survivors.
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

// -------------------------------
// I/O: one JSON object per line, so a partial write is still readable and
// a large corpus never has to be held fully in memory to append to it.
// -------------------------------

/// Appends `cases` to `path` as JSON lines, creating the file (and its
/// parent directory) if it does not exist yet.
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

/// Reads every prepared case out of a JSON-lines file written by
/// `write_prepared_cases`. Blank lines are skipped; a malformed line is an
/// error rather than being silently dropped, since a benchmark reading a
/// corrupted dataset file should fail loudly, not run on fewer cases than
/// it thinks it has.
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
        // The real RegexLib HTML-tag pattern that motivated
        // `MAX_AREGEX_NODES`: its annotated expression passes a million
        // nodes within about fifty characters despite `simp` running after
        // every derivative.
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
        // Fails on the very first character (1 of 10): not a worst case,
        // whatever it claims. A short string can't test this: length 1
        // makes "failed after all of it" and "failed immediately" the same
        // fraction, so this needs a string long enough to tell them apart.
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
        write_prepared_cases(&path, std::slice::from_ref(&case)).unwrap(); // append

        let read_back = read_prepared_cases(&path).unwrap();
        assert_eq!(read_back.len(), 2);
        assert_eq!(read_back[0].pattern, case.pattern);
        assert_eq!(read_back[0].input, case.input);
        assert_eq!(read_back[0].verified_match, case.verified_match);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pathologically_wide_character_class_range_is_rejected() {
        // `-` to en dash: every code point in between, found by running
        // this pipeline against a real RegexLib pattern (ID 3642).
        let pattern = r"[0-9\--– ]{10}";
        let err = core_regex(pattern).expect_err("should be rejected, not translated");
        assert!(err.contains("depth"), "error should explain the depth cap: {err}");
    }

    #[test]
    fn ordinary_real_patterns_with_several_classes_are_not_rejected() {
        // Both were wrongly rejected by an earlier, total-size-based
        // version of this cap: several separate, ordinary-sized classes
        // add up to a few hundred nodes but nest only shallowly.
        assert!(core_regex(r"^[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+)*@(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?\.)+(?:[a-zA-Z]{2}|aero|asia|biz|cat|com|coop|edu|gov|info|int|jobs|mil|mobi|museum|name|net|org|pro|tel|travel)$").is_ok());
        assert!(core_regex(r"^([\+][0-9]{1,3}([ \.\-])?)?([\(]{1}[0-9]{3}[\)])?([0-9A-Z \.\-]{1,32})((x|ext|extension)?[0-9]{1,4}?)$").is_ok());
    }

    #[test]
    fn an_ordinary_pattern_is_well_under_the_node_cap() {
        let r = core_regex(r"[a-c]+\d{2}").expect("should translate");
        assert!(node_count(&r) < 100);
    }
}
