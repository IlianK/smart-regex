//! regex-engine/src/cli/mod.rs
//!
//! CLI module for regex matching and parsing
//!
//! Usage:
//!   cargo run -- <COMMAND> <REGEX> <INPUT> [OPTIONS]
//!
//! Commands:
//!   match    Boolean match only (returns exit code 0/1)
//!   parse    POSIX parsing with parse tree output
//!
//! Options (per-subcommand -- run `cargo run -- match --help` /
//! `cargo run -- parse --help` to see them listed with their allowed
//! values directly):
//!   match:
//!     --matcher <MATCHER>   naive, deriv, pderiv, all   (default: deriv)
//!     --diag <DIAG>         0, 1, 2, 3                  (default: 0)
//!   parse:
//!     --parser <PARSER>     deriv_rec, deriv_loop, deriv_bc, pderiv,
//!                           pderiv_bc, all               (default: deriv_rec)
//!     --diag <DIAG>         0, 1, 2, 3                  (default: 0)
//!     --diag-report <PATH>  Level-3 report destination (default: stdout,
//!                           or reports/report.txt if --diag 3 with no
//!                           path given)

mod input;
mod matcher;
mod parser;

use clap::{Parser, Subcommand, ValueEnum};
use regex_engine::matchers::MatcherType;
use regex_engine::parsers::ParserType;
use regex_engine::diagnostics::DiagLevel;
use matcher::{run_match_single, run_match_all};
use parser::{run_parse_single, run_parse_all};

// -------------------------------
// CLI Parser
// -------------------------------

#[derive(Parser)]
#[command(name = "regex-engine")]
#[command(about = "Regular expression matching and parsing", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Boolean match only (returns exit code 0/1)
    Match {
        /// Regular expression pattern
        regex: String,
        /// Input string to match
        input: String,
        /// Matcher to use
        #[arg(long, value_enum, default_value_t = MatcherArg::Deriv)]
        matcher: MatcherArg,
        /// Diagnostic verbosity level
        #[arg(long, value_enum, default_value_t = DiagArg::Off)]
        diag: DiagArg,
    },
    /// POSIX parsing with parse tree output
    Parse {
        /// Regular expression pattern
        regex: String,
        /// Input string to parse
        input: String,
        /// Parser to use
        #[arg(long, value_enum, default_value_t = ParserArg::DerivRec)]
        parser: ParserArg,
        /// Diagnostic verbosity level
        #[arg(long, value_enum, default_value_t = DiagArg::Off)]
        diag: DiagArg,
        /// Level-3 report destination (only meaningful with --diag 3;
        /// defaults to reports/report.txt if omitted)
        #[arg(long)]
        diag_report: Option<String>,
    },
}

// -------------------------------
// --matcher: naive, deriv, pderiv, all
// -------------------------------

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum MatcherArg {
    Naive,
    Deriv,
    Pderiv,
    All,
}

impl MatcherArg {
    /// The single MatcherType this resolves to, or None for `all` (handled
    /// separately by run_match_all, which always runs all three).
    fn single(self) -> Option<MatcherType> {
        match self {
            MatcherArg::Naive  => Some(MatcherType::Naive),
            MatcherArg::Deriv  => Some(MatcherType::Deriv),
            MatcherArg::Pderiv => Some(MatcherType::PDeriv),
            MatcherArg::All    => None,
        }
    }
}

// -------------------------------
// --parser: deriv_rec, deriv_loop, deriv_bc, pderiv, pderiv_bc, all
// -------------------------------

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ParserArg {
    #[value(name = "deriv_rec")]
    DerivRec,
    #[value(name = "deriv_loop")]
    DerivLoop,
    #[value(name = "deriv_bc")]
    DerivBc,
    /// Alias of pderiv_bc -- the reference gives only a bit-coded
    /// partial-derivative construction, no independent non-bitcoded one
    /// (see regex::pderiv::annotated), so both resolve to the same parser.
    Pderiv,
    #[value(name = "pderiv_bc")]
    PderivBc,
    All,
}

impl ParserArg {
    /// The single ParserType this resolves to, or None for `all` (handled
    /// separately by run_parse_all).
    fn single(self) -> Option<ParserType> {
        match self {
            ParserArg::DerivRec  => Some(ParserType::DerivRec),
            ParserArg::DerivLoop => Some(ParserType::DerivLoop),
            ParserArg::DerivBc   => Some(ParserType::DerivBC),
            ParserArg::Pderiv    => Some(ParserType::PDeriv),
            ParserArg::PderivBc  => Some(ParserType::PDerivBC),
            ParserArg::All       => None,
        }
    }
}

// -------------------------------
// --diag: 0, 1, 2, 3
// -------------------------------

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum DiagArg {
    #[value(name = "0")]
    Off,
    #[value(name = "1")]
    Basic,
    #[value(name = "2")]
    Verbose,
    #[value(name = "3")]
    Debug,
}

impl From<DiagArg> for DiagLevel {
    fn from(d: DiagArg) -> DiagLevel {
        match d {
            DiagArg::Off     => DiagLevel::Off,
            DiagArg::Basic   => DiagLevel::Basic,
            DiagArg::Verbose => DiagLevel::Verbose,
            DiagArg::Debug   => DiagLevel::Debug,
        }
    }
}

// -------------------------------
// Main
// -------------------------------

pub fn run() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Match { regex, input, matcher, diag } => {
            match matcher.single() {
                Some(m) => run_match_single(&regex, &input, m, diag.into()),
                None    => run_match_all(&regex, &input),
            }
        }
        Commands::Parse { regex, input, parser, diag, diag_report } => {
            match parser.single() {
                Some(p) => run_parse_single(&regex, &input, p, diag.into(), diag_report),
                None    => run_parse_all(&regex, &input),
            }
        }
    }
}
