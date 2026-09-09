//! Parser selection via the REGEX_PARSER env var or CLI; see docs/PARSERS.md for the five values.

use crate::types::{Regex, ParseTree};
use super::selection::ParserType;

/// Parse using parser selected by REGEX_PARSER environment variable
pub fn parse(input: &str, r: &Regex) -> Option<ParseTree> {
    ParserType::single_from_env().parser()(input, r)
}
