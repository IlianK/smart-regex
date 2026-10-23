//! src/data/prepare.rs
//!
//! Pattern preparation: parse, translate, and tag a raw pattern's
//! `MatchIntent`, with the anchor-stripping and depth/node caps that
//! `run_pipeline` (`mod.rs`) and `dataset_stats` both rely on.
//!
//! `core_regex_tagged` accepts one anchor shape this pipeline recognizes:
//! a top-level `^...$` wrap. The anchors are stripped before translation,
//! since they are redundant under full-string matching. Any other anchor
//! placement still reaches `translate` and is rejected.

use crate::data::types::{MatchIntent, PreparedCase};
use crate::frontend::{
    case_fold, parse_ext_pattern, strip_dot_newline, strip_pcre_delimiters, translate, ExtPat,
};
use crate::parsers::deriv_bc::deriv::deriv_bc;
use crate::parsers::deriv_bc::internalize::internalize;
use crate::parsers::deriv_bc::nullable::{is_phi, nullable_bc};
use crate::parsers::deriv_bc::simplify::simp;
use crate::types::{ARegex, Regex};

/// Cap on the bit-coded annotated expression's node count, so a single
/// pathological (pattern, input) pair can't make `matches_bounded` hang.
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

/// Whether `input` matches `r`, via the bit-coded engine; `None` past the
/// node cap rather than risking the blowup an unbounded check could hit.
pub fn matches_bounded(input: &str, r: &Regex) -> Option<bool> {
    let mut ri = internalize(r);
    if aregex_node_count(&ri) > MAX_AREGEX_NODES {
        return None;
    }
    for c in input.chars() {
        ri = simp(deriv_bc(ri, c));
        if aregex_node_count(&ri) > MAX_AREGEX_NODES {
            return None;
        }
        if is_phi(&ri) {
            return Some(false);
        }
    }
    Some(nullable_bc(&ri))
}

/// Core regex for a PCRE-rule body with `MatchIntent`, plus depth/node caps.
/// Strips a top-level `^...$` wrap before `translate` sees it.
pub fn core_regex_tagged(raw_pattern: &str) -> Result<(Regex, MatchIntent), String> {
    let (body, flags) = strip_pcre_delimiters(raw_pattern);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    let ep = if flags.dot_all { ep } else { strip_dot_newline(&ep) };
    let (ep, stripped) = strip_full_anchors(ep);

    // `stripped` false means no top-level `^...$` wrap -> pattern has lone `^`/`$` somewhere
    // `translate` rejects it below, discarding this binding
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

    // Any anchor not strippable shape reaches `translate` unchanged and is rejected there
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

/// Strips a top-level `^...$` wrap -- the one shape whose anchors are
/// provably redundant under full-string matching; returns whether it did.
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

/// Where `^`/`$` sit in a pattern's top-level shape; diagnostic only.
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

/// Append `cases` to `path` as JSON lines, creating parents if needed.
pub fn write_prepared_cases(path: &std::path::Path, cases: &[PreparedCase]) -> std::io::Result<()> {
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

/// Read every prepared case from a JSON-lines file written by `write_prepared_cases`.
/// Blank lines skipped.
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
    use crate::data::types::{PreparedCase, SourceKind};

    fn core(pattern: &str) -> Regex {
        let ep = crate::frontend::parse_ext_pattern(pattern).expect("should parse");
        translate(&ep).expect("should translate")
    }

    #[test]
    fn matches_bounded_confirms_a_real_match() {
        let r = core(r"[a-c]+\d{2}");
        assert_eq!(matches_bounded("ab12", &r), Some(true));
    }

    #[test]
    fn matches_bounded_confirms_a_real_non_match() {
        let r = core(r"[a-c]+\d{2}");
        assert_eq!(matches_bounded("zzz", &r), Some(false));
    }

    #[test]
    fn prepared_cases_round_trip_through_jsonl() {
        let dir = std::env::temp_dir().join(format!("data-pipeline-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cases.jsonl");
        let _ = std::fs::remove_file(&path);

        let case = PreparedCase {
            source: SourceKind::S2ELab,
            pattern: r"^\d{5}$".to_string(),
            input: "12345".to_string(),
            expected_match: true,
            match_intent: MatchIntent::ExplicitFullMatch,
        };
        write_prepared_cases(&path, std::slice::from_ref(&case)).unwrap();
        write_prepared_cases(&path, std::slice::from_ref(&case)).unwrap();

        let read_back = read_prepared_cases(&path).unwrap();
        assert_eq!(read_back.len(), 2);
        assert_eq!(read_back[0].pattern, case.pattern);
        assert_eq!(read_back[0].input, case.input);
        assert_eq!(read_back[0].expected_match, case.expected_match);

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
}
