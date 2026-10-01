//! src/frontend/mod.rs
//!
//! External surface-syntax pattern frontend. Entry points differ in how
//! much is applied on top of the shared parse-and-translate core; see
//! `FRONTEND.md` for the comparison.

pub mod alphabet;
pub mod ext_pattern;
pub mod parse;
pub mod translate;

pub use ext_pattern::{case_fold, strip_dot_newline, ExtPat};
pub use parse::parse_ext_pattern;
pub use translate::translate;

use crate::types::Regex;
use ext_pattern::{contains_carat_anywhere, contains_dollar_anywhere, ends_with_dollar, starts_with_carat};
use translate::wildcard_run;

/// Flags from a `/PATTERN/FLAGS` wrapper. Only flags that change what the
/// pattern denotes are recorded; unknown flags are ignored.
///
/// - Buffer selectors (`U`, `H`, `P`, `C`, ...) name which buffer to
///   match and how to normalize it. They do not change what the pattern
///   denotes; recorded nowhere, never rejected.
/// - `R` (relative) starts the search after the previous `content:`
///   match. Not visible from the pattern string, so it is recorded but
///   not rejected; treated as an ordinary search pattern.
/// - `A` (anchored) matches at the start of the buffer, like a leading
///   `^`. Visible from the pattern string, so handled: left padding
///   suppressed.
/// - `E` (dollar-endonly) forbids `$` from matching before a trailing
///   newline. Handled: without `E`, a `$`-anchored end allows one
///   optional trailing `\n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PcreFlags {
    pub case_insensitive: bool,
    pub multiline: bool,
    pub dot_all: bool,
    pub relative: bool,
    pub extended: bool,
    pub anchored: bool,
    pub dollar_endonly: bool,
}

impl PcreFlags {
    /// Parse the flag string after the closing `/`. Flags not tracked
    /// here are silently ignored.
    fn parse(flags: &str) -> Self {
        let f = |c: char| flags.contains(c);
        PcreFlags {
            case_insensitive: f('i'),
            multiline: f('m'),
            dot_all: f('s'),
            relative: f('R'),
            extended: f('x'),
            anchored: f('A'),
            dollar_endonly: f('E'),
        }
    }

    /// A recorded flag this frontend cannot honour, as a reason string.
    /// `None` if every recorded flag is handled or has no effect on the
    /// translated `Regex`. Only `m` and `x` are rejected.
    pub fn unsupported(&self) -> Option<&'static str> {
        if self.multiline {
            return Some(
                "multiline ('m'): '^' and '$' would match at newlines as well as at \
                 buffer boundaries, which the search-padding scheme does not represent",
            );
        }
        if self.extended {
            return Some(
                "extended ('x'): whitespace in the pattern is not significant, which \
                 this parser does not model",
            );
        }
        None
    }
}

/// Parse a pattern for full-string matching.
pub fn parse_pattern(s: &str) -> Result<Regex, String> {
    let ep = parse_ext_pattern(s)?;
    translate(&ep)
}

/// Parse a pattern for substring search: `Σ*` padding on any side not
/// guarded by `^`/`$`.
pub fn parse_dataset_pattern(s: &str) -> Result<Regex, String> {
    let (body, flags) = strip_pcre_delimiters(s);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    let ep = if flags.dot_all { ep } else { strip_dot_newline(&ep) };
    translate_as_search(&ep, flags.anchored, flags.dollar_endonly)
}

/// Parse a `/PATTERN/FLAGS` PCRE rule for substring search: case-folds on
/// `i`, excludes `\n` from `.` unless `s`, treats `A` like a leading `^`,
/// allows one trailing `\n` after `$` unless `E`. Rejects `m` and `x`.
pub fn parse_pcre_rule(s: &str) -> Result<Regex, String> {
    let (body, flags) = strip_pcre_delimiters(s);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    let ep = if flags.dot_all { ep } else { strip_dot_newline(&ep) };
    translate_as_search(&ep, flags.anchored, flags.dollar_endonly)
}

/// Report whether `s` parses and which anchors it carries, including `A`
/// (which anchors the start like a leading `^`).
pub fn detect_anchors(s: &str) -> Result<(bool, bool), String> {
    let (body, flags) = strip_pcre_delimiters(s);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    Ok((starts_with_carat(&ep) || flags.anchored, ends_with_dollar(&ep)))
}

/// Padding scheme. `force_left_anchor` is the `A` flag (suppresses left
/// padding like a leading `^`). `dollar_endonly` is the `E` flag: without
/// it, PCRE allows one optional trailing `\n` after a `$`-anchored end.
fn translate_as_search(
    ep: &ExtPat,
    force_left_anchor: bool,
    dollar_endonly: bool,
) -> Result<Regex, String> {
    let core = translate(ep)?;
    let start_anchored = starts_with_carat(ep) || force_left_anchor;
    let end_anchored = ends_with_dollar(ep);
    let core = if end_anchored && !dollar_endonly {
        Regex::seq(core, Regex::alt(Regex::Eps, Regex::lit('\n')))
    } else {
        core
    };
    Ok(match (start_anchored, end_anchored) {
        (true, true) => core,
        (true, false) => Regex::seq(core, wildcard_run()),
        (false, true) => Regex::seq(wildcard_run(), core),
        (false, false) => Regex::seq(Regex::seq(wildcard_run(), core), wildcard_run()),
    })
}

/// Strip a `/PATTERN/FLAGS` wrapper. A string not starting with `/` is
/// returned unchanged with default (all-false) flags.
pub fn strip_pcre_delimiters(s: &str) -> (&str, PcreFlags) {
    let s = s.trim();
    if !s.starts_with('/') {
        return (s, PcreFlags::default());
    }
    match s.rfind('/') {
        Some(end) if end > 0 => {
            let body = &s[1..end];
            let flags = &s[end + 1..];
            (body, PcreFlags::parse(flags))
        }
        _ => (s, PcreFlags::default()),
    }
}

/// A known way an accepted pattern's translated `Regex` fails to denote
/// exactly what the source pattern means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaithfulnessGap {
    /// `R`: the match position is relative to the rule's previous
    /// `content:` match, which the frontend cannot see from the pattern
    /// string alone. Translated as an ordinary search pattern.
    Relative,
    /// `^` or `$` present somewhere but not at its own top-level edge (or
    /// not on every branch of the alternation it sits in). `translate`
    /// lowers it to `Eps` rather than enforcing or rejecting it.
    NestedAnchor,
}

/// Every way `pattern`'s translation, if accepted, is a known
/// approximation rather than faithful. Empty if there is no known gap.
/// Does not itself check whether `pattern` is accepted; see
/// `parse_pcre_rule` or `is_faithful`. A buffer-selector flag is
/// deliberately not a gap: it says where to test the pattern, not what
/// it denotes.
pub fn faithfulness_gaps(pattern: &str) -> Vec<FaithfulnessGap> {
    let mut gaps = Vec::new();
    let (body, flags) = strip_pcre_delimiters(pattern);
    if flags.relative {
        gaps.push(FaithfulnessGap::Relative);
    }
    if let Ok(ep) = parse_ext_pattern(body) {
        let start = starts_with_carat(&ep) || flags.anchored;
        let end = ends_with_dollar(&ep);
        if (!start && contains_carat_anywhere(&ep)) || (!end && contains_dollar_anywhere(&ep)) {
            gaps.push(FaithfulnessGap::NestedAnchor);
        }
    }
    gaps
}

/// Whether `pattern` is faithful: accepted by `parse_pcre_rule` and with
/// no known approximation.
pub fn is_faithful(pattern: &str) -> bool {
    parse_pcre_rule(pattern).is_ok() && faithfulness_gaps(pattern).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::parse_deriv_std_rec;

    #[test]
    fn eps_and_never_end_to_end() {
        let r = parse_dataset_pattern("^aε$").expect("should parse");
        assert!(parse_deriv_std_rec("a", &r).is_some());

        let r = parse_dataset_pattern("^∅$").expect("should parse");
        assert!(parse_deriv_std_rec("", &r).is_none());
        assert!(parse_deriv_std_rec("x", &r).is_none());
    }

    #[test]
    fn parse_dataset_pattern_end_to_end() {
        let r = parse_dataset_pattern(r"[a-c]+\d{2}").expect("should parse");
        assert!(parse_deriv_std_rec("aabb12", &r).is_some());
        assert!(parse_deriv_std_rec("aabb1", &r).is_none());
    }

    #[test]
    fn parse_dataset_pattern_rejects_backreference() {
        assert!(parse_dataset_pattern(r"(a)\1").is_err());
    }

    #[test]
    fn parse_dataset_pattern_rejects_lookahead() {
        assert!(parse_dataset_pattern(r"a(?=b)").is_err());
    }

    #[test]
    fn parse_dataset_pattern_rejects_word_boundary() {
        assert!(parse_dataset_pattern(r"a\bb").is_err());
    }

    #[test]
    fn strip_pcre_delimiters_extracts_body_and_flags() {
        let (body, flags) = strip_pcre_delimiters("/abc/i");
        assert_eq!(body, "abc");
        assert!(flags.case_insensitive);

        let (body, flags) = strip_pcre_delimiters("/abc/");
        assert_eq!(body, "abc");
        assert_eq!(flags, PcreFlags::default());

        let (body, flags) = strip_pcre_delimiters("abc");
        assert_eq!(body, "abc");
        assert_eq!(flags, PcreFlags::default());
    }

    #[test]
    fn strip_pcre_delimiters_records_relative_flag() {
        let (body, flags) = strip_pcre_delimiters("/abc/R");
        assert_eq!(body, "abc");
        assert!(flags.relative);
    }

    #[test]
    fn parse_pcre_rule_case_folds_on_i_flag() {
        let r = parse_pcre_rule("/abc/i").expect("should parse");
        assert!(parse_deriv_std_rec("ABC", &r).is_some());
        assert!(parse_deriv_std_rec("aBc", &r).is_some());
        assert!(parse_deriv_std_rec("abc", &r).is_some());
    }

    #[test]
    fn parse_pcre_rule_without_i_flag_is_case_sensitive() {
        let r = parse_pcre_rule("/abc/").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &r).is_some());
        assert!(parse_deriv_std_rec("ABC", &r).is_none());
    }

    #[test]
    fn parse_pcre_rule_passes_through_bare_pattern() {
        let r = parse_pcre_rule("abc").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &r).is_some());
    }

    #[test]
    fn parse_pcre_rule_rejects_multiline_flag() {
        let err = parse_pcre_rule("/^abc/m").expect_err("m should be rejected");
        assert!(err.contains("multiline"), "error should mention multiline: {err}");
    }

    #[test]
    fn parse_pcre_rule_rejects_extended_flag() {
        let err = parse_pcre_rule("/a b c/x").expect_err("x should be rejected");
        assert!(err.contains("extended"), "error should mention extended: {err}");
    }

    #[test]
    fn parse_pcre_rule_accepts_relative_flag() {
        assert!(parse_pcre_rule("/abc/R").is_ok());
    }

    #[test]
    fn parse_pcre_rule_accepts_dot_all_flag() {
        assert!(parse_pcre_rule("/a.c/s").is_ok());
    }

    #[test]
    fn dot_without_s_excludes_newline_but_s_restores_it() {
        use crate::parsers::parse_deriv_std_rec;

        let without_s = parse_pcre_rule("/a.c/").expect("should parse");
        assert_eq!(parse_deriv_std_rec("a\nc", &without_s), None);
        assert!(parse_deriv_std_rec("abc", &without_s).is_some());

        let with_s = parse_pcre_rule("/a.c/s").expect("should parse");
        assert!(parse_deriv_std_rec("a\nc", &with_s).is_some());
    }

    #[test]
    fn a_flag_anchors_the_start_like_a_leading_carat() {
        use crate::parsers::parse_deriv_std_rec;
        // Per Suricata's own docs: "A pattern has to match at the
        // beginning of a buffer. (In pcre ^ is similar to A.)"
        let anchored = parse_pcre_rule("/abc/A").expect("should parse");
        assert_eq!(parse_deriv_std_rec("ZZZabc", &anchored), None);
        assert!(parse_deriv_std_rec("abc", &anchored).is_some());
        assert!(parse_deriv_std_rec("abcZZZ", &anchored).is_some());

        assert_eq!(detect_anchors("/abc/A").unwrap(), (true, false));
    }

    #[test]
    fn dollar_without_e_allows_one_trailing_newline_but_e_forbids_it() {
        use crate::parsers::parse_deriv_std_rec;
        // Per PCRE's docs (PCRE_DOLLAR_ENDONLY): without 'E', "a
        // dollar also matches immediately before a newline at the end
        // of the string (but not before any other newlines)".
        let lenient = parse_pcre_rule("/abc$/").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &lenient).is_some());
        assert!(parse_deriv_std_rec("abc\n", &lenient).is_some());
        assert_eq!(parse_deriv_std_rec("abc\n\n", &lenient), None);
        assert_eq!(parse_deriv_std_rec("abcX", &lenient), None);

        let strict = parse_pcre_rule("/abc$/E").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &strict).is_some());
        assert_eq!(parse_deriv_std_rec("abc\n", &strict), None);
    }

    #[test]
    fn is_faithful_true_for_an_ordinary_pattern() {
        assert!(is_faithful("/abc/"));
        assert!(is_faithful("/^abc$/i"));
    }

    #[test]
    fn is_faithful_false_for_a_rejected_pattern() {
        // Not accepted at all, so not faithful either 
        assert!(!is_faithful("/abc/m"));
        assert!(!is_faithful("/(a)(b)\\1/"));
    }

    #[test]
    fn is_faithful_false_for_relative_flag() {
        assert!(!is_faithful("/abc/R"));
        assert_eq!(faithfulness_gaps("/abc/R"), vec![FaithfulnessGap::Relative]);
    }

    #[test]
    fn is_faithful_false_for_nested_anchor() {
        assert!(!is_faithful("/a(^b)*c/"));
        assert_eq!(faithfulness_gaps("/a(^b)*c/"), vec![FaithfulnessGap::NestedAnchor]);
    }

    #[test]
    fn is_faithful_true_with_a_buffer_selector_flag_alone() {
        // A buffer selector says where to test the pattern, not what it
        // means, so it does not disqualify a pattern from faithful.
        assert!(is_faithful("/abc/U"));
        assert!(faithfulness_gaps("/abc/U").is_empty());
    }

    #[test]
    fn parse_pcre_rule_ignores_buffer_modifier_flags() {
        // `U`, `H`, `P` are Snort's buffer selectors, not PCRE flags.
        assert!(parse_pcre_rule("/abc/U").is_ok());
        assert!(parse_pcre_rule("/abc/H").is_ok());
        assert!(parse_pcre_rule("/abc/P").is_ok());
    }

    #[test]
    fn detect_anchors_rejects_unsupported_flags() {
        assert!(detect_anchors("/^abc/m").is_err());
        assert!(detect_anchors("/^abc/i").is_ok());
        assert!(detect_anchors("/^abc/R").is_ok());
    }

    #[test]
    fn unanchored_pattern_matches_as_substring() {
        let r = parse_dataset_pattern("abc").expect("should parse");
        assert!(parse_deriv_std_rec("xxabcyy", &r).is_some());
        assert!(parse_deriv_std_rec("abc", &r).is_some());
        assert!(parse_deriv_std_rec("xyz", &r).is_none());
    }

    #[test]
    fn unanchored_nullable_pattern_matches_anything() {
        let r = parse_dataset_pattern("a*").expect("should parse");
        assert!(parse_deriv_std_rec("", &r).is_some());
        assert!(parse_deriv_std_rec("xyz", &r).is_some());
        assert!(parse_deriv_std_rec("zzz999", &r).is_some());
    }

    #[test]
    fn fully_anchored_pattern_rejects_surrounding_text() {
        let r = parse_dataset_pattern("^abc$").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &r).is_some());
        assert!(parse_deriv_std_rec("xxabcyy", &r).is_none());
        assert!(parse_deriv_std_rec("xabc", &r).is_none());
        assert!(parse_deriv_std_rec("abcx", &r).is_none());
    }

    #[test]
    fn start_anchored_only_allows_trailing_text_not_leading() {
        let r = parse_dataset_pattern("^abc").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &r).is_some());
        assert!(parse_deriv_std_rec("abcxyz", &r).is_some());
        assert!(parse_deriv_std_rec("xabc", &r).is_none());
    }

    #[test]
    fn end_anchored_only_allows_leading_text_not_trailing() {
        let r = parse_dataset_pattern("abc$").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &r).is_some());
        assert!(parse_deriv_std_rec("xyzabc", &r).is_some());
        assert!(parse_deriv_std_rec("abcx", &r).is_none());
    }
}