//! regex-engine/src/posix/parser.rs
//! 
//! POSIX parser dispatch - selects which parser implementation to use
//!
//! Parser selection via REGEX_PARSER environment variable:
//!   REGEX_PARSER=recursive  (default) - standard recursive parser
//!   REGEX_PARSER=loop                   - standard loop parser
//!   REGEX_PARSER=bitcoded               - bit-coded parser

use crate::types::{Regex, ParseTree};
use crate::posix::standard::{parse_recursive, parse_loop};
use crate::posix::bitcoded::parse_bitcoded;
use super::selection::ParserType;

/// Parse using parser selected by REGEX_PARSER environment variable
pub fn parse_posix(input: &str, r: &Regex) -> Option<ParseTree> {
    match ParserType::single_from_env() {
        ParserType::Recursive => parse_recursive(input, r),
        ParserType::Loop => parse_loop(input, r),
        ParserType::Bitcoded => parse_bitcoded(input, r),
    }
}

pub fn match_posix(input: &str, r: &Regex) -> bool {
    parse_posix(input, r).is_some()
}