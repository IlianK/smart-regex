//! External surface-syntax pattern frontend.

pub mod alphabet;
pub mod ext_pattern;
pub mod parse;
pub mod translate;

pub use ext_pattern::{case_fold, ExtPat};
pub use parse::parse_ext_pattern;
pub use translate::translate;

use crate::types::Regex;
use ext_pattern::{ends_with_dollar, starts_with_carat};
use translate::wildcard_run;

/// Flags read from a `/PATTERN/FLAGS` wrapper. Only the ones whose
/// presence changes the meaning of the pattern in a way this frontend
/// can represent are recorded; unknown flags are ignored.
///
/// Several flags that appear in Snort and SpamAssassin rules are
/// deliberately not recorded and not rejected:
///
/// - Snort's buffer-selection and normalization flags (`U`, `H`, `P`,
///   `C`, `D`, `I`, `K`, `M`, `G`, `A`, `B`, `O`, `S`) tell Snort *which
///   buffer* to match and how to normalize it. They do not change what
///   the pattern denotes.
/// - `R` (relative) tells Snort where in the buffer to start the search
///   -- after the previous `content:` match, not from the buffer start.
///   That is a positioning constraint the frontend cannot see, since it
///   receives only the pattern string, not the surrounding rule. The
///   frontend treats an `R` pattern as an ordinary search pattern and
///   documents that simplification; it does not reject it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PcreFlags {
    pub case_insensitive: bool,
    pub multiline: bool,
    pub dot_all: bool,
    pub relative: bool,
    pub extended: bool,
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
    translate_as_search(&ep)
}

/// Parse a `/PATTERN/FLAGS` PCRE rule for substring search. Case-folds
/// when the `i` flag is present. Rejects patterns whose flags (`m`, `x`)
/// would change what `^` / `$` mean or how the pattern is tokenized.
/// Inputs not delimited by `/` are passed through unchanged.
pub fn parse_pcre_rule(s: &str) -> Result<Regex, String> {
    let (body, flags) = strip_pcre_delimiters(s);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    translate_as_search(&ep)
}

/// Report whether `s` parses and which anchors it carries. Returns an
/// error for patterns whose flags this frontend does not support, so the
/// caller sees the same set of patterns `parse_pcre_rule` accepts.
pub fn detect_anchors(s: &str) -> Result<(bool, bool), String> {
    let (body, flags) = strip_pcre_delimiters(s);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    Ok((starts_with_carat(&ep), ends_with_dollar(&ep)))
}

fn translate_as_search(ep: &ExtPat) -> Result<Regex, String> {
    let core = translate(ep)?;
    Ok(match (starts_with_carat(ep), ends_with_dollar(ep)) {
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
        // `s` is harmless: Dot already expands to an alphabet including '\n'.
        assert!(parse_pcre_rule("/a.c/s").is_ok());
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