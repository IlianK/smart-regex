//! External surface-syntax pattern frontend.

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

/// Flags read from a `/PATTERN/FLAGS` wrapper. Only the ones whose
/// presence changes the meaning of the pattern in a way this frontend
/// can represent are recorded; unknown flags are ignored.
///
/// Several flags that appear in Snort and SpamAssassin rules are
/// deliberately not recorded and not rejected:
///
/// - Snort's buffer-selection and normalization flags (`U`, `H`, `P`,
///   `C`, `D`, `I`, `K`, `M`, `G`, `B`, `O`, `S`) tell Snort *which
///   buffer* to match and how to normalize it. They do not change what
///   the pattern denotes.
/// - `R` (relative) tells Snort where in the buffer to start the search
///   -- after the previous `content:` match, not from the buffer start.
///   That is a positioning constraint the frontend cannot see, since it
///   receives only the pattern string, not the surrounding rule. The
///   frontend treats an `R` pattern as an ordinary search pattern and
///   documents that simplification; it does not reject it.
///
/// `A` is handled, not ignored: per PCRE/Suricata's own documentation,
/// "a pattern has to match at the beginning of a buffer. (In pcre `^`
/// is similar to `A`.)" It is a positional constraint the frontend
/// *can* see (unlike `R`, it needs no context beyond the pattern
/// string), so `/foo/A` is treated exactly like `/^foo/`: left padding
/// is suppressed the same way an explicit leading `^` suppresses it.
///
/// `E` is handled too: per PCRE's own documentation (`PCRE_DOLLAR_ENDONLY`),
/// without it "a dollar also matches immediately before a newline at the
/// end of the string (but not before any other newlines)". With it (the
/// default assumed by everything else in this frontend before this flag
/// was read), `$` matches only at the true end. So a pattern ending in
/// `$` without `/E` is translated to allow one optional trailing `\n`
/// after the core that a strict `$` would not; see
/// `translate_as_search`.
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
    /// Parse a flag string (the part after the closing `/`). A flag that
    /// is present but not tracked here is silently ignored; this mirrors
    /// the previous behaviour for anything other than `i`.
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

    /// A flag this frontend cannot honour. Returned as the reason string
    /// for an error. `None` if every recorded flag is either handled or
    /// has no effect on the translated `Regex`.
    ///
    /// Only `m` and `x` are rejected. `R` is recorded but not rejected:
    /// it is a positioning constraint the frontend cannot see, and
    /// treating an `R` pattern as an ordinary search pattern is a
    /// documented simplification (see the struct's doc comment). The
    /// buffer-modifier flags (`U`, `H`, `P`, `C`, ...) are not recorded
    /// at all and never reach this method.
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

/// Parse a pattern for substring search. Adds `Σ*` padding on any side
/// not guarded by `^` / `$`. Rejects patterns whose PCRE flags (`m`, `x`)
/// would change what `^` / `$` mean or how the pattern is tokenized.
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

/// Parse a `/PATTERN/FLAGS` PCRE rule for substring search. Case-folds
/// when the `i` flag is present, excludes `\n` from `.` unless the
/// dotall (`s`) flag is present, suppresses left padding when the `A`
/// (anchored) flag is present (the same as an explicit leading `^`), and
/// allows one optional trailing `\n` after a `$`-anchored end unless the
/// `E` (dollar-endonly) flag is present. Rejects patterns whose flags
/// (`m`, `x`) would change what `^` / `$` mean or how the pattern is
/// tokenized. Inputs not delimited by `/` are passed through unchanged.
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

/// Report whether `s` parses and which anchors it carries -- including
/// the `A` flag, which anchors the start exactly like a leading `^`.
/// Returns an error for patterns whose flags this frontend does not
/// support, so the caller sees the same set of patterns `parse_pcre_rule`
/// accepts.
pub fn detect_anchors(s: &str) -> Result<(bool, bool), String> {
    let (body, flags) = strip_pcre_delimiters(s);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    Ok((starts_with_carat(&ep) || flags.anchored, ends_with_dollar(&ep)))
}

/// `force_left_anchor` is the `A` PCRE flag: it suppresses left padding
/// exactly like a leading `^`, without the pattern needing to contain
/// one. `dollar_endonly` is the `E` PCRE flag: when a `$` anchors the
/// end and `E` is absent, PCRE's own semantics allow one optional
/// trailing `\n` after it (`PCRE_DOLLAR_ENDONLY`'s documented default:
/// "a dollar also matches immediately before a newline at the end of
/// the string"); this frontend used to always require a strict end
/// (equivalent to `E` always being set) regardless of the flag.
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

/// Strip a `/PATTERN/FLAGS` wrapper, returning the body and the flags.
/// A string not starting with `/` is returned with default (all-false)
/// flags.
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

/// A specific, known way an *accepted* pattern's translated `Regex` can
/// fail to denote exactly what the source pattern means. See
/// `faithfulness_gaps`. These are the only two gaps this frontend still
/// has, as of the review that added this type: every other PCRE
/// construct this frontend accepts is translated exactly, and everything
/// else is rejected outright rather than silently approximated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaithfulnessGap {
    /// `R`: the match position is relative to the rule's previous
    /// `content:` match. The frontend sees only the pattern string, not
    /// the surrounding rule, so it cannot see that constraint; the
    /// pattern is translated as an ordinary search pattern instead.
    Relative,
    /// `^` or `$` appears somewhere in the pattern but not at its own
    /// top-level edge (or not on every branch of the alternation it
    /// sits in), so `translate` silently lowers it to `Eps` rather than
    /// enforcing it or rejecting the pattern.
    NestedAnchor,
}

/// Every way `pattern`'s translation, if accepted, is a known
/// approximation of what the pattern means rather than exactly faithful
/// to it. Empty for a pattern with no known gap. This function does not
/// itself check whether `pattern` is accepted at all; call
/// `parse_pcre_rule` (or use `is_faithful`, which checks both) for that.
/// A buffer-selector flag (`U`/`H`/`P`/...) is deliberately not a gap
/// here: it says which buffer the pattern should be tested against, not
/// what the pattern itself denotes as a regular language.
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

/// Whether `pattern` is faithful: `parse_pcre_rule` accepts it, and the
/// resulting `Regex` denotes exactly what the pattern means, with no
/// known approximation (`faithfulness_gaps` is empty). A pattern
/// `parse_pcre_rule` rejects outright is not faithful either -- there is
/// no `Regex` for it to be faithful or unfaithful about.
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
        // `R` is a positioning constraint the frontend cannot see; it is
        // recorded but not rejected. Treating an R pattern as an ordinary
        // search pattern is the documented simplification.
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

        // Per PCRE's own docs (PCRE_DOLLAR_ENDONLY): without 'E', "a
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
        // Not accepted at all, so not faithful either -- there is no
        // Regex for it to be faithful or unfaithful about.
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