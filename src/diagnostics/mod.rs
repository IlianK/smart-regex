//! Diagnostics module: verbosity levels 0-3 for `parse`, controlled via --diag. `match` has none.

pub mod replay;
pub mod format;
pub mod diag_1;
pub mod diag_2;
pub mod diag_3;
pub mod report;

use crate::types::Regex;
use crate::parsers::selection::ParserType;

// DiagLevel

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

// DiagConfig

#[derive(Debug, Clone)]
pub struct DiagConfig {
    pub level:       DiagLevel,
    pub parser_type: ParserType,
    pub report_path: Option<String>,
}

impl DiagConfig {
    pub fn new(
        level: DiagLevel,
        parser_type: ParserType,
        report_path: Option<String>,
    ) -> Self {
        Self { level, parser_type, report_path }
    }

    pub fn read_from_env() -> Self {
        let level        = DiagLevel::from_env();
        let parser_type  = ParserType::single_from_env();

        // Default report path: reports/report.txt
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

// Entry points

/// Run parser and show diagnostics output at level
pub fn run_parser(regex_str: &str, r: &Regex, input: &str, config: &DiagConfig) {
    match config.level {
        DiagLevel::Off     => level0_parser(r, input, config),
        DiagLevel::Basic   => diag_1::run_parser(regex_str, r, input, config),
        DiagLevel::Verbose => diag_2::run_parser(regex_str, r, input, config),
        DiagLevel::Debug   => diag_3::run_parser(regex_str, r, input, config),
    }
}

// Level 0 helper (true/false only)

fn level0_parser(r: &Regex, input: &str, config: &DiagConfig) {
    let matched = config.parser_type.parser()(input, r).is_some();
    println!("{}", matched);
}
