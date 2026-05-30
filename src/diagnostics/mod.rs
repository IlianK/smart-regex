//! Diagnostics module
//!
//! Controls output verbosity for matching and parsing via REGEX_DIAG env var.
//!
//! Levels:
//!   0 = Off     — true / false only
//!   1 = Basic   — regex, input, result, parse tree, error caret
//!   2 = Verbose — Basic + time, expression count, construction steps
//!   3 = Debug   — Verbose + full structural derivation trace (written to
//!                 REGEX_DIAG_REPORT if set, otherwise stdout)
//!
//! Usage:
//!   REGEX_DIAG=1 cargo run -- parse "a*" "aaa"
//!   REGEX_DIAG=2 REGEX_PARSER=bitcoded cargo run -- parse "a*" "aab"
//!   REGEX_DIAG=3 REGEX_DIAG_REPORT=report.txt cargo run -- parse "a*" "aab"
//!   REGEX_DIAG=1 cargo run --example demo_posix

pub mod trace;
pub mod replay;
pub mod level_1;
pub mod level_2;
pub mod level_3;
pub mod report;

use crate::types::Regex;
use crate::posix::selection::ParserType;

// ============================================================================
// DiagLevel
// ============================================================================

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

// ============================================================================
// DiagConfig
// ============================================================================

/// Holds the active diagnostics configuration for a single parse/match call.
#[derive(Debug, Clone)]
pub struct DiagConfig {
    pub level:       DiagLevel,
    /// Which parser is active (determines standard vs. bitcoded output paths)
    pub parser_type: ParserType,
    /// Optional file path for Level 3 report output (REGEX_DIAG_REPORT)
    pub report_path: Option<String>,
}

impl DiagConfig {
    pub fn read_from_env() -> Self {
        let level       = DiagLevel::from_env();
        let parser_type = ParserType::single_from_env();
        let report_path = std::env::var("REGEX_DIAG_REPORT").ok();
        Self { level, parser_type, report_path }
    }

    pub fn is_bitcoded(&self) -> bool {
        self.parser_type == ParserType::Bitcoded
    }

    pub fn is_off(&self) -> bool {
        self.level == DiagLevel::Off
    }
}

// ============================================================================
// Public convenience entry points
// ============================================================================

/// Run a parser and emit diagnostics output at the configured level.
/// This is the single call site used by both the CLI and demos.
pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.level {
        DiagLevel::Off     => level0_parser(r, input, config),
        DiagLevel::Basic   => level_1::run_parser(regex_str, r, input, config),
        DiagLevel::Verbose => level_2::run_parser(regex_str, r, input, config),
        DiagLevel::Debug   => level_3::run_parser(regex_str, r, input, config),
    }
}

/// Run a matcher and emit diagnostics output at the configured level.
pub fn run_matcher(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.level {
        DiagLevel::Off     => level0_matcher(r, input),
        DiagLevel::Basic   => level_1::run_matcher(regex_str, r, input),
        DiagLevel::Verbose => level_1::run_matcher(regex_str, r, input), // matchers have no construction steps
        DiagLevel::Debug   => level_1::run_matcher(regex_str, r, input),
    }
}

// ============================================================================
// Level 0 helpers (inline — no sub-module needed)
// ============================================================================

fn level0_parser(r: &Regex, input: &str, config: &DiagConfig) {
    use crate::posix::standard::{parse_recursive, parse_loop};
    use crate::posix::bitcoded::parse_bitcoded;

    let matched = match config.parser_type {
        ParserType::Recursive => parse_recursive(input, r).is_some(),
        ParserType::Loop      => parse_loop(input, r).is_some(),
        ParserType::Bitcoded  => parse_bitcoded(input, r).is_some(),
    };
    println!("{}", matched);
}

fn level0_matcher(r: &Regex, input: &str) {
    use crate::matchers::{match_naive, match_deriv, match_pderiv};
    use crate::matchers::MatcherType;

    let matched = match MatcherType::single_from_env() {
        MatcherType::Naive  => match_naive(input, r),
        MatcherType::Deriv  => match_deriv(input, r),
        MatcherType::PDeriv => match_pderiv(input, r),
    };
    println!("{}", matched);
}