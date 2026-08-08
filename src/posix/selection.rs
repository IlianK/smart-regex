//! regex-engine/src/posix/selection.rs
//!
//!  Parser selection logic shared between library and CLI

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserType {
    /// standard, deriv, recursive
    DerivRec,
    /// standard, deriv, loop
    DerivLoop,
    /// bitcoded, deriv
    DerivBC,
    /// standard, pderiv - loop-only, no recursive sibling
    PDeriv,
    /// bitcoded, pderiv - loop-only (future work, see regex::pderiv::annotated)
    PDerivBC,
}

impl ParserType {
    pub fn name(&self) -> &'static str {
        match self {
            ParserType::DerivRec  => "recursive",
            ParserType::DerivLoop => "loop",
            ParserType::DerivBC   => "bitcoded",
            ParserType::PDeriv    => "pderiv",
            ParserType::PDerivBC  => "pderiv_bitcoded",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ParserType::DerivRec  => "RECURSIVE",
            ParserType::DerivLoop => "LOOP",
            ParserType::DerivBC   => "BITCODED",
            ParserType::PDeriv    => "PDERIV",
            ParserType::PDerivBC  => "PDERIV-BITCODED",
        }
    }

    /// "all" intentionally stays limited to the three deriv-based parsers.
    /// pderiv/pderiv_bitcoded are opt-in only selections until they're implemented.
    pub fn from_env() -> Vec<ParserType> {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("recursive")       => vec![ParserType::DerivRec],
            Ok("loop")            => vec![ParserType::DerivLoop],
            Ok("bitcoded")        => vec![ParserType::DerivBC],
            Ok("pderiv")          => vec![ParserType::PDeriv],
            Ok("pderiv_bitcoded") => vec![ParserType::PDerivBC],
            Ok("all") => vec![ParserType::DerivRec, ParserType::DerivLoop, ParserType::DerivBC],
            _ => vec![ParserType::DerivRec],
        }
    }

    pub fn single_from_env() -> ParserType {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("loop")            => ParserType::DerivLoop,
            Ok("bitcoded")        => ParserType::DerivBC,
            Ok("pderiv")          => ParserType::PDeriv,
            Ok("pderiv_bitcoded") => ParserType::PDerivBC,
            _ => ParserType::DerivRec,
        }
    }

    /// Single source of truth mapping a selected parser to its concrete
    /// (untraced) parsing function. Every non-traced call site should go
    /// through this rather than re-matching ParserType itself.
    pub fn parser(&self) -> fn(&str, &crate::types::Regex) -> Option<crate::types::ParseTree> {
        match self {
            ParserType::DerivRec  => crate::posix::parse_recursive,
            ParserType::DerivLoop => crate::posix::parse_loop,
            ParserType::DerivBC   => crate::posix::parse_bitcoded,
            ParserType::PDeriv    => {
                |_, _| unimplemented!("pderiv-based standard POSIX parsing - not yet implemented")
            }
            ParserType::PDerivBC  => {
                |_, _| unimplemented!("pderiv-based bit-coded POSIX parsing - future work")
            }
        }
    }
}
