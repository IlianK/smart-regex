//! External surface-syntax pattern AST, modeled on https://github.com/luzhuomi/regex-pderiv.

pub mod alphabet;
pub mod ext_pattern;
pub mod parse;
pub mod translate;

pub use ext_pattern::{ExtPat, case_fold};
pub use parse::parse_ext_pattern;
pub use translate::translate;

use crate::types::Regex;
use ext_pattern::{ends_with_dollar, starts_with_carat};
use translate::wildcard_run;


/// Parse a pattern string into crate's core `Regex`
pub fn parse_pattern(s: &str) -> Result<Regex, String> {
    let ep = parse_ext_pattern(s)?;
    translate(&ep)
}

pub fn parse_dataset_pattern(s: &str) -> Result<Regex, String> {
    let ep = parse_ext_pattern(s)?;
    translate_as_search(&ep)
}

pub fn parse_pcre_rule(s: &str) -> Result<Regex, String> {
    let (body, case_insensitive) = strip_pcre_delimiters(s);
    let ep = parse_ext_pattern(body)?;
    let ep = if case_insensitive { case_fold(&ep) } else { ep };
    translate_as_search(&ep)
}

/// `translate(ep)`, padded with unanchored-search wildcard on whichever side lacks `^`/`$`
fn translate_as_search(ep: &ExtPat) -> Result<Regex, String> {
    let core = translate(ep)?;
    Ok(match (starts_with_carat(ep), ends_with_dollar(ep)) {
        (true, true) => core,
        (true, false) => Regex::seq(core, wildcard_run()),
        (false, true) => Regex::seq(wildcard_run(), core),
        (false, false) => Regex::seq(Regex::seq(wildcard_run(), core), wildcard_run()),
    })
}

/// Strips a `/PATTERN/FLAGS` PCRE-rule wrapper
/// returning the bare pattern body and whether the `i` (case-insensitive) flag was set
/// Passes `s`through unchanged (not case-insensitive) if it isn't `/`-delimited
pub fn strip_pcre_delimiters(s: &str) -> (&str, bool) {
    let s = s.trim();
    if !s.starts_with('/') {
        return (s, false);
    }
    match s.rfind('/') {
        Some(end) if end > 0 => {
            let body = &s[1..end];
            let flags = &s[end + 1..];
            (body, flags.contains('i'))
        }
        _ => (s, false),
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::parse_deriv_std_rec;

    #[test]
    fn eps_and_never_end_to_end() {
        // Definition 1's own eps/never notation, expressible through this frontend
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
    fn strip_pcre_delimiters_extracts_body_and_flag() {
        assert_eq!(strip_pcre_delimiters("/abc/i"), ("abc", true));
        assert_eq!(strip_pcre_delimiters("/abc/"), ("abc", false));
        assert_eq!(strip_pcre_delimiters("abc"), ("abc", false));
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

    // Substring search: unanchored patterns match a *containing* string

    #[test]
    fn unanchored_pattern_matches_as_substring() {
        let r = parse_dataset_pattern("abc").expect("should parse");
        assert!(parse_deriv_std_rec("xxabcyy", &r).is_some());
        assert!(parse_deriv_std_rec("abc", &r).is_some());
        assert!(parse_deriv_std_rec("xyz", &r).is_none());
    }

    // A pattern nullable on empty input (e.g. "a*") is padded on both sides too, so it trivially matches anything.
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
