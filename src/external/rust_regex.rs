//! Thin wrapper around the `regex` crate (Rust's de facto standard engine),
//! used only for benchmarking against this crate's own parsers on the same
//! data. `regex` matches leftmost-first (Perl-like, like Greedy here), with
//! no POSIX leftmost-longest mode of its own -- see docs/EXTERNAL_ENGINES.md.

pub struct RustRegex(regex::Regex);

impl RustRegex {
    pub fn new(pattern: &str, case_insensitive: bool) -> Result<Self, regex::Error> {
        regex::RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map(RustRegex)
    }

    /// Unanchored substring search -- this project's dataset "search"
    /// semantics (docs/testing/DATASETS.md).
    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }

    /// Anchored, whole-string match -- this project's CLI/`parse_pattern`
    /// membership semantics.
    pub fn full_match(&self, text: &str) -> bool {
        matches!(self.0.find(text), Some(m) if m.start() == 0 && m.end() == text.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_and_matches() {
        let re = RustRegex::new("ab*c", false).unwrap();
        assert!(re.full_match("abc"));
        assert!(re.is_match("xxabcyy"));
        assert!(!re.full_match("xxabcyy"));
        assert!(!re.is_match("xyz"));
    }

    #[test]
    fn rejects_invalid_pattern() {
        assert!(RustRegex::new("a(", false).is_err());
    }

    /// `regex` matches leftmost-first, like this project's Greedy parsers
    /// -- unlike RE2, it has no POSIX leftmost-longest mode to select.
    #[test]
    fn leftmost_first_like_greedy() {
        let re = RustRegex::new("a|ab", false).unwrap();
        let m = re.0.find("ab").unwrap();
        assert_eq!((m.start(), m.end()), (0, 1));
    }
}
