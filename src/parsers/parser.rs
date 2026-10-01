//! src/parsers/parser.rs

use crate::types::{Regex, ParseTree};
use super::selection::ParserType;

/// Parse using parser selected by REGEX_PARSER environment variable
pub fn parse(input: &str, r: &Regex) -> Option<ParseTree> {
    ParserType::single_from_env().parser()(input, r)
}
