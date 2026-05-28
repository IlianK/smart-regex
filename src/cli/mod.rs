//! CLI module for regex matching and parsing
//!
//! Usage:
//!   cargo run -- <COMMAND> <REGEX> <INPUT> [OPTIONS]
//!
//! Commands:
//!   match    Boolean match only (returns exit code 0/1)
//!   parse    POSIX parsing with parse tree output
//!
//! Options:
//!   --matcher <MATCHER>    For match command: naive, deriv, pderiv, all (default: deriv)
//!
//! Environment Variables:
//!   REGEX_MATCHER    Matcher: naive, deriv, pderiv, all
//!   REGEX_PARSER     Parser: recursive (default), loop, bitcoded, all
//!   REGEX_DIAG       Diagnostic level: 0=off, 1=basic, 2=diagnostic, 3=debug

mod input;
mod matcher;
mod parser;

use clap::{Parser, Subcommand};
use regex_engine::matchers::MatcherType;
use matcher::{run_match_single, run_match_all};
use parser::{run_parse_single, run_parse_all};

// ============================================================================
// CLI Parser
// ============================================================================

#[derive(Parser)]
#[command(name = "regex-engine")]
#[command(about = "Regular expression matching and parsing", long_about = None)]
struct Cli {
    /// Matcher to use for match command (naive, deriv, pderiv, all)
    #[arg(long, default_value = "deriv")]
    matcher: String,

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
    },
    /// POSIX parsing with parse tree output
    Parse {
        /// Regular expression pattern
        regex: String,
        /// Input string to parse
        input: String,
    },
}

// ============================================================================
// Main
// ============================================================================

pub fn run() {
    let cli = Cli::parse();
    
    // Set environment for diagnostics (parse_posix will read this)
    let diag = std::env::var("REGEX_DIAG").unwrap_or_default();
    std::env::set_var("REGEX_DIAG", &diag);
    
    match cli.command {
        Commands::Match { regex, input } => {
            // CLI --matcher flag overrides environment variable
            let matchers = MatcherType::from_str(&cli.matcher);
            
            if matchers.len() == 1 {
                run_match_single(&regex, &input, matchers[0]);
            } else {
                run_match_all(&regex, &input);
            }
        }
        Commands::Parse { regex, input } => {
            // Parser selection from environment variable (REGEX_PARSER)
            let parsers = regex_engine::posix::ParserType::from_env();
            
            if parsers.len() == 1 {
                run_parse_single(&regex, &input);
            } else {
                run_parse_all(&regex, &input);
            }
        }
    }
}