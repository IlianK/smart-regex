//! src/frontend/mod.rs
//!
//! External surface-syntax pattern frontend.

pub mod alphabet;
pub mod ext_pattern;
pub mod parse;
pub mod translate;

pub use ext_pattern::{case_fold, strip_dot_newline, ExtPat};
pub use parse::parse_ext_pattern;
pub use translate::translate;

use crate::types::Regex;

/// Flags from a `/PATTERN/FLAGS` wrapper.
/// Only flags that change what the pattern denotes are recorded;
/// unknown flags are ignored.
///
/// Buffer selectors (`U`, `H`, `P`, `C`, ...) name which buffer to match
/// and how to normalize it.
/// They do not change what the pattern denotes: recorded nowhere, never
/// rejected.
///
/// `R` (relative), `A` (anchored), `E` (dollar-endonly) exist only to
/// position or tune a search/anchor this frontend has no concept of
/// (`^`/`$` are rejected outright -- see `translate`).
/// None of the three are recorded; a pattern carrying one parses exactly
/// as it would without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PcreFlags {
    pub case_insensitive: bool,
    pub multiline: bool,
    pub dot_all: bool,
    pub extended: bool,
}

impl PcreFlags {
    /// Parse the flag string after the closing `/`.
    /// Flags not tracked here are silently ignored.
    fn parse(flags: &str) -> Self {
        let f = |c: char| flags.contains(c);
        PcreFlags {
            case_insensitive: f('i'),
            multiline: f('m'),
            dot_all: f('s'),
            extended: f('x'),
        }
    }

    /// A recorded flag this frontend cannot honour, as a reason string.
    /// `None` if every recorded flag is handled or has no effect.
    /// Only `m` and `x` are rejected.
    pub fn unsupported(&self) -> Option<&'static str> {
        if self.multiline {
            return Some(
                "multiline ('m'): '^' and '$' would match at newlines as well as at \
                 buffer boundaries, and this frontend does not support anchors at all",
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

/// Parse a `/PATTERN/FLAGS` PCRE rule.
/// Case-folds on `i`; excludes `\n` from `.` unless `s`.
/// Rejects `m` and `x`.
/// Rejects `^`/`$` wherever they appear (via `translate`): this frontend
/// has no anchor or search-padding concept, only full-string matching.
pub fn parse_pcre_rule(s: &str) -> Result<Regex, String> {
    let (body, flags) = strip_pcre_delimiters(s);
    if let Some(reason) = flags.unsupported() {
        return Err(format!("unsupported PCRE flag: {reason}"));
    }
    let ep = parse_ext_pattern(body)?;
    let ep = if flags.case_insensitive { case_fold(&ep) } else { ep };
    let ep = if flags.dot_all { ep } else { strip_dot_newline(&ep) };
    translate(&ep)
}

/// Strip a `/PATTERN/FLAGS` wrapper.
/// A string not starting with `/` is returned unchanged with default
/// (all-false) flags.
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
        let r = parse_pattern("aε").expect("should parse");
        assert!(parse_deriv_std_rec("a", &r).is_some());

        let r = parse_pattern("∅").expect("should parse");
        assert!(parse_deriv_std_rec("", &r).is_none());
        assert!(parse_deriv_std_rec("x", &r).is_none());
    }

    #[test]
    fn parse_pcre_rule_end_to_end() {
        let r = parse_pcre_rule(r"[a-c]+\d{2}").expect("should parse");
        assert!(parse_deriv_std_rec("aabb12", &r).is_some());
        assert!(parse_deriv_std_rec("aabb1", &r).is_none());
    }

    #[test]
    fn parse_pcre_rule_rejects_backreference() {
        assert!(parse_pcre_rule(r"(a)\1").is_err());
    }

    #[test]
    fn parse_pcre_rule_rejects_lookahead() {
        assert!(parse_pcre_rule(r"a(?=b)").is_err());
    }

    #[test]
    fn parse_pcre_rule_rejects_word_boundary() {
        assert!(parse_pcre_rule(r"a\bb").is_err());
    }

    #[test]
    fn parse_pcre_rule_rejects_leading_anchor() {
        let err = parse_pcre_rule("^abc").expect_err("^ should be rejected");
        assert!(err.contains("anchor"), "error should mention anchor: {err}");
    }

    #[test]
    fn parse_pcre_rule_rejects_trailing_anchor() {
        assert!(parse_pcre_rule("abc$").is_err());
    }

    #[test]
    fn parse_pcre_rule_rejects_nested_anchor() {
        assert!(parse_pcre_rule("a(^b)*c").is_err());
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
        let err = parse_pcre_rule("/abc/m").expect_err("m should be rejected");
        assert!(err.contains("multiline"), "error should mention multiline: {err}");
    }

    #[test]
    fn parse_pcre_rule_rejects_extended_flag() {
        let err = parse_pcre_rule("/a b c/x").expect_err("x should be rejected");
        assert!(err.contains("extended"), "error should mention extended: {err}");
    }

    #[test]
    fn parse_pcre_rule_accepts_dot_all_flag() {
        assert!(parse_pcre_rule("/a.c/s").is_ok());
    }

    #[test]
    fn dot_without_s_excludes_newline_but_s_restores_it() {
        let without_s = parse_pcre_rule("/a.c/").expect("should parse");
        assert_eq!(parse_deriv_std_rec("a\nc", &without_s), None);
        assert!(parse_deriv_std_rec("abc", &without_s).is_some());

        let with_s = parse_pcre_rule("/a.c/s").expect("should parse");
        assert!(parse_deriv_std_rec("a\nc", &with_s).is_some());
    }

    #[test]
    fn parse_pcre_rule_ignores_buffer_modifier_flags() {
        assert!(parse_pcre_rule("/abc/U").is_ok());
        assert!(parse_pcre_rule("/abc/H").is_ok());
        assert!(parse_pcre_rule("/abc/P").is_ok());
    }

    #[test]
    fn parse_pcre_rule_ignores_now_meaningless_flags() {
        assert!(parse_pcre_rule("/abc/R").is_ok());
        assert!(parse_pcre_rule("/abc/A").is_ok());
        assert!(parse_pcre_rule("/abc/E").is_ok());
    }

    #[test]
    fn full_string_match_only_no_substring_search() {
        let r = parse_pcre_rule("/abc/").expect("should parse");
        assert!(parse_deriv_std_rec("abc", &r).is_some());
        assert!(parse_deriv_std_rec("xxabcyy", &r).is_none());
        assert!(parse_deriv_std_rec("xabc", &r).is_none());
        assert!(parse_deriv_std_rec("abcx", &r).is_none());
    }
}