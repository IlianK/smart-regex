//! regex-engine/src/posix/parser.rs
//!
//! POSIX parser dispatch - selects which parser implementation to use
//!
//! Parser selection via REGEX_PARSER environment variable:
//!   REGEX_PARSER=recursive  (default) - standard, deriv, recursive
//!   REGEX_PARSER=loop                 - standard, deriv, loop
//!   REGEX_PARSER=bitcoded              - bitcoded, deriv
//!   REGEX_PARSER=pderiv                - standard, pderiv (not yet implemented)
//!   REGEX_PARSER=pderiv_bitcoded       - bitcoded, pderiv (future work)

use crate::types::{Regex, ParseTree};
use super::selection::ParserType;

/// Parse using parser selected by REGEX_PARSER environment variable
pub fn parse_posix(input: &str, r: &Regex) -> Option<ParseTree> {
    ParserType::single_from_env().parser()(input, r)
}
