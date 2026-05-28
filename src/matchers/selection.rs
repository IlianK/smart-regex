//! Matcher selection logic shared between library and CLI

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatcherType {
    Naive,
    Deriv,
    PDeriv,
}

impl MatcherType {
    pub fn name(&self) -> &'static str {
        match self {
            MatcherType::Naive => "naive",
            MatcherType::Deriv => "deriv",
            MatcherType::PDeriv => "pderiv",
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            MatcherType::Naive => "NAIVE",
            MatcherType::Deriv => "DERIV",
            MatcherType::PDeriv => "PDERIV",
        }
    }
    
    pub fn from_str(s: &str) -> Vec<MatcherType> {
        match s {
            "naive" => vec![MatcherType::Naive],
            "deriv" => vec![MatcherType::Deriv],
            "pderiv" => vec![MatcherType::PDeriv],
            "all" => vec![MatcherType::Naive, MatcherType::Deriv, MatcherType::PDeriv],
            _ => vec![MatcherType::Deriv],
        }
    }
    
    pub fn from_env() -> Vec<MatcherType> {
        match std::env::var("REGEX_MATCHER").as_deref() {
            Ok("naive") => vec![MatcherType::Naive],
            Ok("deriv") => vec![MatcherType::Deriv],
            Ok("pderiv") => vec![MatcherType::PDeriv],
            Ok("all") => vec![MatcherType::Naive, MatcherType::Deriv, MatcherType::PDeriv],
            _ => vec![MatcherType::Deriv],
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