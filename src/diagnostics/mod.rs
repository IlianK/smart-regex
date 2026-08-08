//! Diagnostics module
//!
//! Controls output verbosity for matching and parsing via REGEX_DIAG env var.
//!
//! Levels:
//!   0 = Off     - true / false only
//!   1 = Basic   - regex, input, result, parse tree, error caret
//!   2 = Verbose - Basic + time, expression count, construction steps
//!   3 = Debug   - full structural derivation trace
//!                 (written to REGEX_DIAG_REPORT if set, otherwise stdout)
//!
//! Usage:
//!   REGEX_DIAG=1 cargo run -- parse "a*" "aaa"
//!   REGEX_DIAG=2 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aab"
//!   REGEX_DIAG=3 REGEX_DIAG_REPORT=report.txt cargo run -- parse "a*" "aab"
//!   REGEX_DIAG=1 cargo run --example demo_posix

pub mod trace;
pub mod replay;
pub mod level1;
pub mod level2;
pub mod level3;
pub mod report;

use crate::types::Regex;
use crate::posix::selection::ParserType;


// -------------------------------
// DiagLevel
// -------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagLevel {
    /// Level 0: true / false only
    Off     = 0,
    /// Level 1: regex, input, match, tree, error caret
    Basic   = 1,
    /// Level 2: Basic + time, expression count, construction steps
    Verbose = 2,
    /// Level 3: full derivation trace; writes to REGEX_DIAG_REPORT if set
    Debug   = 3,
}

impl DiagLevel {
    pub fn from_env() -> Self {
        match std::env::var("REGEX_DIAG").as_deref() {
            Ok("1") => DiagLevel::Basic,
            Ok("2") => DiagLevel::Verbose,
            Ok("3") => DiagLevel::Debug,
            _       => DiagLevel::Off,
        }
    }
}


// -------------------------------
// DiagConfig
// -------------------------------

/// Holds  diagnostics configfor a single parse/match call
#[derive(Debug, Clone)]
pub struct DiagConfig {
    pub level:       DiagLevel,
    /// Which parser is active (standard vs. bitcoded output paths)
    pub parser_type: ParserType,
    /// Optional file path for Level 3 report output (REGEX_DIAG_REPORT)
    pub report_path: Option<String>,
}

impl DiagConfig {
    pub fn read_from_env() -> Self {
        let level       = DiagLevel::from_env();
        let parser_type = ParserType::single_from_env();

        // Default report path: reports/report.txt
        // Can be overridden with REGEX_DIAG_REPORT=path/to/file.txt
        let report_path = if level == DiagLevel::Debug {
            Some(
                std::env::var("REGEX_DIAG_REPORT")
                    .unwrap_or_else(|_| "reports/report.txt".to_string())
            )
        } else {
            std::env::var("REGEX_DIAG_REPORT").ok()
        };

        Self { level, parser_type, report_path }
    }

    pub fn is_off(&self) -> bool {
        self.level == DiagLevel::Off
    }
}


// -------------------------------
// Entry points
// -------------------------------

/// Run a parser and show diagnostics output at configured level (by CLI and demo)
pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.level {
        DiagLevel::Off     => level0_parser(r, input, config),
        DiagLevel::Basic   => level1::run_parser(regex_str, r, input, config),
        DiagLevel::Verbose => level2::run_parser(regex_str, r, input, config),
        DiagLevel::Debug   => level3::run_parser(regex_str, r, input, config),
    }
}

/// Run a matcher and show diagnostics output at configured level (by CLI and demo)
/// (Levels 2 and 3 fall back to Level 1 => matchers produce no construction steps)
pub fn run_matcher(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.level {
        DiagLevel::Off => level0_matcher(r, input),
        _              => level1::run_matcher(regex_str, r, input),
    }
}


// -------------------------------
// Level 0 helpers
// -------------------------------

fn level0_parser(r: &Regex, input: &str, config: &DiagConfig) {
    let matched = config.parser_type.parser()(input, r).is_some();
    println!("{}", matched);
}

fn level0_matcher(r: &Regex, input: &str) {
    use crate::matchers::MatcherType;
    // MatcherType::from_env() returns Vec<MatcherType>; take the first element.
    // Default is Deriv when REGEX_MATCHER is unset
    let matcher_type = MatcherType::from_env()
        .into_iter()
        .next()
        .unwrap_or(MatcherType::Deriv);

    use crate::matchers::{match_naive, match_deriv, match_pderiv};
    let matched = match matcher_type {
        MatcherType::Naive  => match_naive(input, r),
        MatcherType::Deriv  => match_deriv(input, r),
        MatcherType::PDeriv => match_pderiv(input, r),
    };
    println!("{}", matched);
}