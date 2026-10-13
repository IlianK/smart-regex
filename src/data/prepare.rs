//! src/data/prepare.rs
//!
//! Verification: every candidate from `generate` is checked against the
//! real parser before becoming a `PreparedCase`.
//! `StructuralNegative` candidates must also fail late.
//!
//! `core_regex_tagged` accepts one anchor shape this pipeline recognizes:
//! a top-level `^...$` wrap.
//! The anchors are stripped before translation.
//! They are redundant under full-string matching.
//! Any other anchor placement still reaches `translate` and is rejected.

use crate::data::types::{Candidate, MatchIntent, PreparedCase, SourceKind};
use crate::frontend::{
    case_fold, parse_ext_pattern, strip_dot_newline, strip_pcre_delimiters, translate, ExtPat,
};
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::internalize::internalize;
use crate::parsers::deriv_bc::nullable::{is_phi, nullable_bc};
use crate::parsers::deriv_bc::simplify::simp;
use crate::types::{ARegex, Regex};

/// Cap on the bit-coded annotated expression's node count.
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

/// `internalize`, then `deriv_bc` + `simp` per character.
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

/// Whether `input` matches `r`, via `step_bc_bounded`.
/// `None` means the check exceeded its own node cap.
pub(crate) fn matches_bounded(input: &str, r: &Regex) -> Option<bool> {
    match step_bc_bounded(input, r) {
        SteppedOutcome::DiedAt(_) => Some(false),
        SteppedOutcome::Finished { nullable } => Some(nullable),
        SteppedOutcome::TooExpensive => None,
    }
}

/// Chars consumed before the match becomes impossible.
/// Full length on a match.
/// `None` when the check exceeds its node cap.
pub fn failed_after_chars(input: &str, r: &Regex) -> Option<usize> {
    match step_bc_bounded(input, r) {
        SteppedOutcome::DiedAt(consumed) => Some(consumed),
        SteppedOutcome::Finished { .. } => Some(input.chars().count()),
        SteppedOutcome::TooExpensive => None,
    }
}

/// A negative must fail at or past this fraction of the input.
/// Earlier failures are discarded.
const LATE_FAILURE_THRESHOLD: f64 = 2.0 / 3.0;

/// Core regex for a PCRE-rule body, plus which `MatchIntent` applied.
/// Duplicates `parse_pcre_rule`'s steps to add depth/node caps.
/// Also strips a top-level `^...$` wrap before `translate` sees it.
pub fn core_regex_tagged(raw_pattern: &str) -> Result<(Regex, MatchIntent), String> {
    let (body, flags) = strip_pcre_delimiters(raw_pattern);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    let ep = if flags.dot_all { ep } else { strip_dot_newline(&ep) };
    let (ep, stripped) = strip_full_anchors(ep);
    // `stripped` false means no top-level `^...$` wrap.
    // The pattern may still have a lone `^`/`$` somewhere.
    // `translate` then rejects it below, discarding this binding.
    let intent = if stripped {
        MatchIntent::ExplicitFullMatch
    } else {
        MatchIntent::UnanchoredAmbiguous
    };

    let estimated_depth = estimated_translated_depth(&ep);
    if estimated_depth > MAX_CORE_DEPTH {
        return Err(format!(
            "pattern is estimated to translate to a nesting depth of at least {estimated_depth}, \
             exceeding this pipeline's cap of {MAX_CORE_DEPTH}; likely a character class whose \
             range spans far more code points than intended (for example `[\\--\u{2013}]`). \
             Rejected before translation, since translating a pattern this deep is itself the risk."
        ));
    }

    // Any anchor not in the one shape `strip_full_anchors` recognizes
    // reaches `translate` unchanged and is rejected there.
    let r = translate(&ep)?;
    let nodes = node_count(&r);
    if nodes > MAX_CORE_NODES {
        return Err(format!(
            "translated pattern has {nodes} nodes, exceeding this pipeline's cap of {MAX_CORE_NODES}"
        ));
    }
    Ok((r, intent))
}

/// Core regex for a PCRE-rule body; discards `MatchIntent`.
pub fn core_regex(raw_pattern: &str) -> Result<Regex, String> {
    core_regex_tagged(raw_pattern).map(|(r, _intent)| r)
}

/// Strips a top-level `^...$` wrap.
/// The one shape whose anchors are provably redundant under full-string
/// matching.
/// Returns `(possibly-stripped ep, true)` if it stripped something.
/// Returns `(ep unchanged, false)` otherwise.
fn strip_full_anchors(ep: ExtPat) -> (ExtPat, bool) {
    match unwrap_shell(ep.clone()) {
        ExtPat::Concat(mut parts)
            if parts.len() >= 2
                && parts.first() == Some(&ExtPat::Carat)
                && parts.last() == Some(&ExtPat::Dollar) =>
        {
            parts.remove(parts.len() - 1);
            parts.remove(0);
            let stripped = if parts.is_empty() {
                ExtPat::Eps
            } else if parts.len() == 1 {
                parts.into_iter().next().expect("checked len == 1")
            } else {
                ExtPat::Concat(parts)
            };
            (stripped, true)
        }
        _ => (ep, false),
    }
}

/// Unwraps `Group`/`GroupNonMarking` shells to find the structure below.
fn unwrap_shell(ep: ExtPat) -> ExtPat {
    match ep {
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => unwrap_shell(*inner),
        other => other,
    }
}

fn top_level_starts_with_carat(ep: &ExtPat) -> bool {
    match unwrap_shell(ep.clone()) {
        ExtPat::Concat(parts) => parts.first() == Some(&ExtPat::Carat),
        ExtPat::Carat => true,
        _ => false,
    }
}

fn top_level_ends_with_dollar(ep: &ExtPat) -> bool {
    match unwrap_shell(ep.clone()) {
        ExtPat::Concat(parts) => parts.last() == Some(&ExtPat::Dollar),
        ExtPat::Dollar => true,
        _ => false,
    }
}

fn contains_anchor(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Carat | ExtPat::Dollar => true,
        ExtPat::Concat(parts) | ExtPat::Or(parts) => parts.iter().any(contains_anchor),
        ExtPat::Group(inner)
        | ExtPat::GroupNonMarking(inner)
        | ExtPat::Opt(inner)
        | ExtPat::Plus(inner)
        | ExtPat::Star(inner) => contains_anchor(inner),
        ExtPat::Bound(inner, _, _) => contains_anchor(inner),
        _ => false,
    }
}

/// Where `^`/`$` sit in a pattern's top-level shape.
/// Diagnostic only.
/// Used by `dataset_stats` to explain "anchor" rejections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AnchorShape {
    /// Never reached `ExtPat`: backreference, lookaround, or syntax error.
    Unparseable,
    /// No `^`/`$` anywhere.
    None,
    /// `^` first and `$` last at the top level; recoverable.
    FullyWrapped,
    /// `^` at the top level; no `$`.
    StartOnly,
    /// `$` at the top level; no `^`.
    EndOnly,
    /// An anchor present, but not in any of the shapes above.
    NestedOrPartial,
}

impl AnchorShape {
    pub fn label(&self) -> &'static str {
        match self {
            AnchorShape::Unparseable => "unparseable",
            AnchorShape::None => "none",
            AnchorShape::FullyWrapped => "fully-wrapped",
            AnchorShape::StartOnly => "start-only",
            AnchorShape::EndOnly => "end-only",
            AnchorShape::NestedOrPartial => "nested",
        }
    }

    /// Fixed order for a shape-distribution table.
    pub const ALL: [AnchorShape; 6] = [
        AnchorShape::Unparseable,
        AnchorShape::None,
        AnchorShape::FullyWrapped,
        AnchorShape::StartOnly,
        AnchorShape::EndOnly,
        AnchorShape::NestedOrPartial,
    ];
}

pub fn anchor_shape(ep: &ExtPat) -> AnchorShape {
    if !contains_anchor(ep) {
        return AnchorShape::None;
    }
    let (_, fully_wrapped) = strip_full_anchors(ep.clone());
    if fully_wrapped {
        return AnchorShape::FullyWrapped;
    }
    match (top_level_starts_with_carat(ep), top_level_ends_with_dollar(ep)) {
        (true, false) => AnchorShape::StartOnly,
        (false, true) => AnchorShape::EndOnly,
        _ => AnchorShape::NestedOrPartial,
    }
}

fn estimated_translated_depth(ep: &ExtPat) -> usize {
    match ep {
        ExtPat::Empty | ExtPat::Eps | ExtPat::Never | ExtPat::Char(_) | ExtPat::Escape(_) => 1,
        ExtPat::Carat | ExtPat::Dollar => 1,
        ExtPat::Dot => 98, // full alphabet Alt chain
        ExtPat::Any(cs) | ExtPat::NoneOf(cs) => cs.len().max(1),
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => estimated_translated_depth(inner),
        ExtPat::Opt(inner) => 1 + estimated_translated_depth(inner),
        ExtPat::Plus(inner) => 1 + 2 * estimated_translated_depth(inner),
        ExtPat::Star(inner) => 1 + estimated_translated_depth(inner),
        ExtPat::Or(parts) => 1 + parts.iter().map(estimated_translated_depth).max().unwrap_or(0),
        ExtPat::Concat(parts) => parts.iter().map(estimated_translated_depth).sum::<usize>() + parts.len(),
        ExtPat::WordBoundary(_) => 1,
        ExtPat::Bound(inner, lo, hi) => {
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

/// Reject before translating if estimated depth exceeds this.
const MAX_CORE_DEPTH: usize = 2_000;

/// Coarser cap on the translated pattern's node count.
const MAX_CORE_NODES: usize = 20_000;

/// Verify one candidate against the real parser.
/// Returns the `PreparedCase` on success, `None` otherwise.
pub fn verify_candidate(
    source: SourceKind,
    raw_pattern: &str,
    candidate: Candidate,
) -> Option<PreparedCase> {
    let (r, match_intent) = core_regex_tagged(raw_pattern).ok()?;
    let verified_match = matches_bounded(&candidate.text, &r)?;
    if verified_match != candidate.claimed_match {
        return None;
    }

    let failed_after = if !verified_match {
        let consumed = failed_after_chars(&candidate.text, &r)?;
        let total = candidate.text.chars().count().max(1);
        if candidate.provenance.requires_late_failure()
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
        provenance: candidate.provenance,
        verified_match,
        failed_after_chars: failed_after,
        match_intent,
    })
}

/// Verify every candidate, keeping survivors in original order.
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

/// Keep exactly `per_side` matches and `per_side` non-matches from one
/// pattern's verified cases.
/// Returns empty if either side has fewer than `per_side`.
/// The caller's `--inputs N` is `2 * per_side`.
///
/// Within each side, a case whose `Provenance::is_from_dataset` is true
/// is kept ahead of one that is not.
/// Stable otherwise, so ties keep their generation order.
/// Innovatorved can mix real classified lines with structural top-up
/// candidates (see `run_pipeline`'s `RuleContext::Innovatorved` branch).
/// This ordering means real ones are never displaced by synthetic ones
/// competing for the same `per_side` slots.
///
/// Every pattern that survives contributes the same total count and the
/// same match/non-match split, which is what makes an aggregate
/// benchmark over them sound.
pub fn balance_cases(cases: Vec<PreparedCase>, per_side: usize) -> Vec<PreparedCase> {
    let mut cases = cases;
    cases.sort_by_key(|c| !c.provenance.is_from_dataset());
    let (matches, non_matches): (Vec<PreparedCase>, Vec<PreparedCase>) =
        cases.into_iter().partition(|c| c.verified_match);
    if matches.len() < per_side || non_matches.len() < per_side {
        return Vec::new();
    }
    matches.into_iter().take(per_side).chain(non_matches.into_iter().take(per_side)).collect()
}

/// Append `cases` to `path` as JSON lines, creating parents if needed.
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

/// Read every prepared case from a JSON-lines file written by
/// `write_prepared_cases`.
/// Blank lines are skipped.
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
            text: "zzz".to_string(),
            provenance: crate::data::types::Provenance::Structural,
            claimed_match: true,
        };
        assert!(verify_candidate(SourceKind::RegexLib, pattern, candidate).is_none());
    }

    #[test]
    fn a_candidate_too_expensive_to_verify_is_dropped_not_hung() {
        let pattern = r#"</?(\w+)(\s*\w*\s*=\s*("[^"]*"|'[^']'|[^>]*))*|/?>"#;
        let candidate = Candidate {
            text: "<wxxzuzzwyyz=}| \n \r\r\r=\t\t \r\t\"}~~|~}\"\n\t\rzzy    \n\n=\r'~'\t \r\t =     \"~~}}~}\"y=\r \n\"~z}}\"\nz \r =\r\t~~~".to_string(),
            provenance: crate::data::types::Provenance::Structural,
            claimed_match: true,
        };
        assert!(verify_candidate(SourceKind::RegexLib, pattern, candidate).is_none());
    }

    #[test]
    fn a_trivially_failing_negative_candidate_is_rejected() {
        let pattern = r"[a-c]+\d{2}";
        let candidate = Candidate {
            text: "zzzzzzzzzz".to_string(),
            provenance: crate::data::types::Provenance::StructuralNegative,
            claimed_match: false,
        };
        assert!(verify_candidate(SourceKind::RegexLib, pattern, candidate).is_none());
    }

    #[test]
    fn an_early_failing_dataset_given_negative_is_also_rejected() {
        let pattern = r"[a-c]+\d{2}";
        let candidate = Candidate {
            text: "zzzzzzzzzz".to_string(),
            provenance: crate::data::types::Provenance::DatasetGiven,
            claimed_match: false,
        };
        assert!(verify_candidate(SourceKind::S2ELab, pattern, candidate).is_none());
    }

    #[test]
    fn an_early_failing_dataset_text_classified_negative_is_also_rejected() {
        let pattern = r"[a-c]+\d{2}";
        let candidate = Candidate {
            text: "zzzzzzzzzz".to_string(),
            provenance: crate::data::types::Provenance::DatasetTextClassified,
            claimed_match: false,
        };
        assert!(verify_candidate(SourceKind::Innovatorved, pattern, candidate).is_none());
    }

    #[test]
    fn a_late_failing_negative_candidate_is_accepted() {
        let pattern = r"[a-c]+\d{2}";
        let mut rng = StdRng::seed_from_u64(7);
        let r = core(pattern);
        let text = generate::sample_negative_late(&r, &mut rng, 4).expect("should generate");
        let candidate = Candidate {
            text,
            provenance: crate::data::types::Provenance::StructuralNegative,
            claimed_match: false,
        };
        let prepared = verify_candidate(SourceKind::RegexLib, pattern, candidate)
            .expect("a genuinely late failure should survive verification");
        assert!(!prepared.verified_match);
        assert!(prepared.failed_after_chars.is_some());
    }

    fn case(verified_match: bool) -> PreparedCase {
        PreparedCase {
            source: SourceKind::RegexLib,
            pattern: "a".to_string(),
            input: "a".to_string(),
            provenance: crate::data::types::Provenance::Structural,
            verified_match,
            failed_after_chars: None,
            match_intent: MatchIntent::UnanchoredAmbiguous,
        }
    }

    fn case_with(
        verified_match: bool,
        provenance: crate::data::types::Provenance,
        input: &str,
    ) -> PreparedCase {
        PreparedCase { input: input.to_string(), provenance, ..case(verified_match) }
    }

    #[test]
    fn balance_keeps_exactly_per_side_of_each_when_both_sides_have_enough() {
        let cases: Vec<PreparedCase> =
            (0..3).map(|_| case(true)).chain((0..4).map(|_| case(false))).collect();
        let balanced = balance_cases(cases, 3);
        assert_eq!(balanced.len(), 6);
        assert_eq!(balanced.iter().filter(|c| c.verified_match).count(), 3);
        assert_eq!(balanced.iter().filter(|c| !c.verified_match).count(), 3);
    }

    #[test]
    fn balance_drops_the_pattern_entirely_when_either_side_is_short() {
        let cases: Vec<PreparedCase> =
            (0..5).map(|_| case(true)).chain((0..2).map(|_| case(false))).collect();
        assert!(balance_cases(cases, 3).is_empty());
    }

    #[test]
    fn balance_drops_a_pattern_with_only_matches_or_only_non_matches() {
        let only_matches: Vec<PreparedCase> = (0..5).map(|_| case(true)).collect();
        assert!(balance_cases(only_matches, 3).is_empty());
        let only_non_matches: Vec<PreparedCase> = (0..5).map(|_| case(false)).collect();
        assert!(balance_cases(only_non_matches, 3).is_empty());
    }

    #[test]
    fn balance_with_exactly_per_side_on_both_sides_keeps_everything() {
        let cases: Vec<PreparedCase> =
            (0..3).map(|_| case(true)).chain((0..3).map(|_| case(false))).collect();
        assert_eq!(balance_cases(cases, 3).len(), 6);
    }

    #[test]
    fn balance_prefers_from_dataset_cases_over_synthetic_ones_within_each_side() {
        use crate::data::types::Provenance;
        let cases = vec![
            case_with(true, Provenance::Structural, "synth-match-1"),
            case_with(true, Provenance::Structural, "synth-match-2"),
            case_with(true, Provenance::DatasetTextClassified, "real-match"),
            case_with(false, Provenance::StructuralNegative, "synth-nonmatch-1"),
            case_with(false, Provenance::StructuralNegative, "synth-nonmatch-2"),
            case_with(false, Provenance::DatasetTextClassified, "real-nonmatch"),
        ];
        let balanced = balance_cases(cases, 2);
        assert_eq!(balanced.len(), 4);
        assert!(balanced.iter().any(|c| c.input == "real-match"));
        assert!(balanced.iter().any(|c| c.input == "real-nonmatch"));
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
            provenance: crate::data::types::Provenance::Structural,
            verified_match: true,
            failed_after_chars: None,
            match_intent: crate::data::types::MatchIntent::UnanchoredAmbiguous,
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
        assert!(core_regex(r"[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-zA-Z0-9!#$%&'*+/=?^_`{|}~-]+)*@(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?\.)+(?:[a-zA-Z]{2}|aero|asia|biz|cat|com|coop|edu|gov|info|int|jobs|mil|mobi|museum|name|net|org|pro|tel|travel)").is_ok());
        assert!(core_regex(r"([\+][0-9]{1,3}([ \.\-])?)?([\(]{1}[0-9]{3}[\)])?([0-9A-Z \.\-]{1,32})((x|ext|extension)?[0-9]{1,4}?)").is_ok());
    }

    #[test]
    fn an_ordinary_pattern_is_well_under_the_node_cap() {
        let r = core_regex(r"[a-c]+\d{2}").expect("should translate");
        assert!(node_count(&r) < 100);
    }

    #[test]
    fn core_regex_rejects_multiline_flag() {
        assert!(core_regex("/^abc/m").is_err());
    }

    #[test]
    fn core_regex_accepts_relative_flag() {
        assert!(core_regex("/abc/R").is_ok());
    }

    #[test]
    fn fully_wrapped_anchors_are_stripped_and_tagged_explicit() {
        let (r, intent) = core_regex_tagged("^abc$").expect("should strip and translate");
        assert_eq!(r, core("abc"));
        assert_eq!(intent, MatchIntent::ExplicitFullMatch);
    }

    #[test]
    fn unanchored_pattern_translates_unchanged_and_is_tagged_ambiguous() {
        let (r, intent) = core_regex_tagged("abc").expect("should translate");
        assert_eq!(r, core("abc"));
        assert_eq!(intent, MatchIntent::UnanchoredAmbiguous);
    }

    #[test]
    fn carat_dollar_only_pattern_strips_to_eps() {
        let (r, intent) = core_regex_tagged("^$").expect("should strip to Eps");
        assert_eq!(r, Regex::Eps);
        assert_eq!(intent, MatchIntent::ExplicitFullMatch);
    }

    #[test]
    fn fully_wrapped_anchors_inside_a_group_are_also_stripped() {
        let (r, intent) = core_regex_tagged("(^abc$)").expect("should strip and translate");
        assert_eq!(r, core("abc"));
        assert_eq!(intent, MatchIntent::ExplicitFullMatch);
    }

    #[test]
    fn start_anchor_only_is_still_rejected() {
        assert!(core_regex_tagged("^abc").is_err());
    }

    #[test]
    fn end_anchor_only_is_still_rejected() {
        assert!(core_regex_tagged("abc$").is_err());
    }

    #[test]
    fn anchor_nested_in_one_alternation_branch_is_still_rejected() {
        assert!(core_regex_tagged("(^a|b)c").is_err());
    }

    #[test]
    fn anchor_under_a_quantifier_is_still_rejected() {
        assert!(core_regex_tagged("(^a)*b").is_err());
    }

    #[test]
    fn stripping_is_specific_to_the_dataset_pipeline_not_the_frontend() {
        assert!(crate::frontend::parse_pattern("^abc$").is_err());
    }

    fn shape_of(pattern: &str) -> AnchorShape {
        let ep = crate::frontend::parse_ext_pattern(pattern).expect("should parse");
        anchor_shape(&ep)
    }

    #[test]
    fn anchor_shape_classifies_every_case() {
        assert_eq!(shape_of("abc"), AnchorShape::None);
        assert_eq!(shape_of("^abc$"), AnchorShape::FullyWrapped);
        assert_eq!(shape_of("(^abc$)"), AnchorShape::FullyWrapped);
        assert_eq!(shape_of("^abc"), AnchorShape::StartOnly);
        assert_eq!(shape_of("abc$"), AnchorShape::EndOnly);
        assert_eq!(shape_of("(^a|b)c"), AnchorShape::NestedOrPartial);
        assert_eq!(shape_of("(^a)*b"), AnchorShape::NestedOrPartial);
        assert_eq!(shape_of("^"), AnchorShape::StartOnly);
        assert_eq!(shape_of("$"), AnchorShape::EndOnly);
        assert_eq!(shape_of("^$"), AnchorShape::FullyWrapped);
    }

    #[test]
    fn verify_candidate_tags_match_intent_on_the_prepared_case() {
        let r = core("abc");
        let candidate = Candidate {
            text: "abc".to_string(),
            provenance: crate::data::types::Provenance::Structural,
            claimed_match: true,
        };
        let prepared = verify_candidate(SourceKind::RegexLib, "^abc$", candidate)
            .expect("should verify");
        assert_eq!(prepared.match_intent, MatchIntent::ExplicitFullMatch);
        assert_eq!(matches_bounded("abc", &r), Some(true));
    }
}