//! regex-engine/src/posix/parser.rs
//!
//! POSIX parser dispatch - selects which parser implementation to use
//!
//! Parser selection via REGEX_PARSER environment variable or CLI:
//!   REGEX_PARSER=deriv_rec   (default) - standard, deriv, recursive
//!   REGEX_PARSER=deriv_loop            - standard, deriv, loop
//!   REGEX_PARSER=deriv_bc              - bitcoded, deriv
//!   REGEX_PARSER=pderiv                - standard, pderiv
//!   REGEX_PARSER=pderiv_bc             - bitcoded, pderiv

use crate::types::{Regex, ParseTree};
use super::selection::ParserType;

/// Parse using parser selected by REGEX_PARSER environment variable
pub fn parse_posix(input: &str, r: &Regex) -> Option<ParseTree> {
    ParserType::single_from_env().parser()(input, r)
}
