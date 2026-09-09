//! Matcher selection logic shared between library and CLI.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatcherType {
    Naive,
    Deriv,
    PDeriv,
}

impl MatcherType {
    pub fn display_name(&self) -> &'static str {
        match self {
            MatcherType::Naive      => "NAIVE",
            MatcherType::Deriv      => "DERIV",
            MatcherType::PDeriv     => "PDERIV",
        }
    }

    pub fn matcher(&self) -> fn(&str, &crate::types::Regex) -> bool {
        match self {
            MatcherType::Naive => crate::matchers::match_naive,
            MatcherType::Deriv => crate::matchers::match_deriv,
            MatcherType::PDeriv => crate::matchers::match_pderiv,
        }
    }
}
