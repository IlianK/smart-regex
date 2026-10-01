//! src/external/rust_regex.rs
//!
//! Wrapper over the `regex` crate, for benchmarking against this crate's
//! own parsers. `regex` matches leftmost-first (like this project's Greedy
//! parsers); it has no POSIX leftmost-longest mode. See
//! docs/EXTERNAL_ENGINES.md.

pub struct RustRegex(regex::Regex);

impl RustRegex {
    pub fn new(pattern: &str, case_insensitive: bool) -> Result<Self, regex::Error> {
        regex::RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map(RustRegex)
    }

    /// Unanchored substring search.
    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }

    /// Whole-string match.
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

    #[test]
    fn leftmost_first_like_greedy() {
        let re = RustRegex::new("a|ab", false).unwrap();
        let m = re.0.find("ab").unwrap();
        assert_eq!((m.start(), m.end()), (0, 1));
    }
}