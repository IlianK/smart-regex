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
//!   match:
//!     --matcher <MATCHER>   naive, deriv, pderiv, all   (default: deriv)
//!   parse:
//!     --parser <PARSER>     deriv_std_rec, deriv_std_loop, deriv_bc, pderiv_std, pderiv_bc, all (default: deriv_std_rec)
//!     --diag <DIAG>         0, 1, 2, 3                  (default: 0)
//!     --diag-report <PATH>  Level-3 report destination 

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
    /// Matcher (returns true/false)
    Match {
        regex: String,
        input: String,
        #[arg(long, value_enum, default_value_t = MatcherArg::Deriv)]
        matcher: MatcherArg,
        /// Pad unanchored sides with Sigma* (substring search) instead of
        /// requiring the whole input to match the whole pattern. Anchors
        /// (^/$) still suppress padding on the side they're on.
        #[arg(long)]
        search: bool,
    },

    /// Parser (returns parse tree)
    Parse {
        regex: String,
        input: String,
        #[arg(long, value_enum, default_value_t = ParserArg::DerivStdRec)]
        parser: ParserArg,
        #[arg(long, value_enum, default_value_t = DiagArg::Off)]
        diag: DiagArg,
        #[arg(long)]
        diag_report: Option<String>,
        /// Pad unanchored sides with Sigma* (substring search) instead of
        /// requiring the whole input to match the whole pattern. Anchors
        /// (^/$) still suppress padding on the side they're on.
        #[arg(long)]
        search: bool,
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
// --parser: deriv_std_rec, deriv_std_loop, deriv_bc, pderiv_std, pderiv_bc
// -------------------------------

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ParserArg {
    #[value(name = "deriv_std_rec")]
    DerivStdRec,
    #[value(name = "deriv_std_loop")]
    DerivStdLoop,
    #[value(name = "deriv_bc")]
    DerivBc,
    #[value(name = "pderiv_std")]
    PderivStd,
    #[value(name = "pderiv_bc")]
    PderivBc,

    All,
} 

impl ParserArg {
    fn single(self) -> Option<ParserType> {
        match self {
            ParserArg::DerivStdRec  => Some(ParserType::DerivStdRec),
            ParserArg::DerivStdLoop => Some(ParserType::DerivStdLoop),
            ParserArg::DerivBc      => Some(ParserType::DerivBc),
            ParserArg::PderivStd    => Some(ParserType::PDerivStd),
            ParserArg::PderivBc     => Some(ParserType::PDerivBc),
            ParserArg::All          => None,
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
        Commands::Match { regex, input, matcher, search } => {
            match matcher.single() {
                Some(m) => run_match_single(&regex, &input, m, search),
                None    => run_match_all(&regex, &input, search),
            }
        }
        Commands::Parse { regex, input, parser, diag, diag_report, search } => {
            match parser.single() {
                Some(p) => run_parse_single(&regex, &input, p, diag.into(), diag_report, search),
                None    => run_parse_all(&regex, &input, search),
            }
        }
    }
}
