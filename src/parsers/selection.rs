//! src/parsers/selection.rs
//! 
//! Parser selection logic shared between library and CLI.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserType {
    DerivStdRec,
    DerivStdLoop,
    DerivBc,
    PDerivStd,
    PDerivBc,
}

impl ParserType {
    pub fn name(&self) -> &'static str {
        match self {
            ParserType::DerivStdRec     => "deriv_std_rec",
            ParserType::DerivStdLoop    => "deriv_std_loop",
            ParserType::DerivBc         => "deriv_bc",
            ParserType::PDerivStd       => "pderiv_std",
            ParserType::PDerivBc        => "pderiv_bc",
        }
    }

    /// Parses one of `name()`'s own strings back into a `ParserType`.
    /// `None` for anything else, including `"all"`: that selects a
    /// comparison mode, not a single parser, and is each caller's own
    /// concern (see `examples/demo_parse.rs`, `src/cli/mod.rs`), not
    /// something this type can represent.
    pub fn parse(s: &str) -> Option<ParserType> {
        match s {
            "deriv_std_rec"  => Some(ParserType::DerivStdRec),
            "deriv_std_loop" => Some(ParserType::DerivStdLoop),
            "deriv_bc"       => Some(ParserType::DerivBc),
            "pderiv_std"     => Some(ParserType::PDerivStd),
            "pderiv_bc"      => Some(ParserType::PDerivBc),
            _ => None,
        }
    }

    pub fn single_from_env() -> ParserType {
        std::env::var("REGEX_PARSER").ok().and_then(|s| ParserType::parse(&s)).unwrap_or(ParserType::DerivStdRec)
    }

    /// Mapping selected parser to (untraced) parsing function
    pub fn parser(&self) -> fn(&str, &crate::types::Regex) -> Option<crate::types::ParseTree> {
        match self {
            ParserType::DerivStdRec     => crate::parsers::parse_deriv_std_rec,
            ParserType::DerivStdLoop    => crate::parsers::parse_deriv_std_loop,
            ParserType::DerivBc         => crate::parsers::parse_deriv_bc,
            ParserType::PDerivStd       => crate::parsers::parse_pderiv_std,
            ParserType::PDerivBc        => crate::parsers::parse_pderiv_bc,
        }
    }
}
