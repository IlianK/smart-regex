//! regex-engine/src/posix/selection.rs
//!
//!  Parser selection logic shared between library and CLI

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserType {
    DerivRec,
    DerivLoop,
    DerivBC,
    PDeriv,
    PDerivBC,
}

impl ParserType {
    pub fn name(&self) -> &'static str {
        match self {
            ParserType::DerivRec  => "deriv_rec",
            ParserType::DerivLoop => "deriv_loop",
            ParserType::DerivBC   => "deriv_bc",
            ParserType::PDeriv    => "pderiv",
            ParserType::PDerivBC  => "pderiv_bc",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ParserType::DerivRec  => "DERIV_REC",
            ParserType::DerivLoop => "DERIV_LOOP",
            ParserType::DerivBC   => "DERIV_BC",
            ParserType::PDeriv    => "PDERIV",
            ParserType::PDerivBC  => "PDERIV_BC",
        }
    }

    pub fn from_env() -> Vec<ParserType> {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("deriv_rec")  => vec![ParserType::DerivRec],
            Ok("deriv_loop") => vec![ParserType::DerivLoop],
            Ok("deriv_bc")   => vec![ParserType::DerivBC],
            Ok("pderiv")     => vec![ParserType::PDeriv],
            Ok("pderiv_bc")  => vec![ParserType::PDerivBC],
            Ok("all") => vec![ParserType::DerivRec, ParserType::DerivLoop, ParserType::DerivBC],
            _ => vec![ParserType::DerivRec],
        }
    }

    pub fn single_from_env() -> ParserType {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("deriv_loop") => ParserType::DerivLoop,
            Ok("deriv_bc")   => ParserType::DerivBC,
            Ok("pderiv")     => ParserType::PDeriv,
            Ok("pderiv_bc")  => ParserType::PDerivBC,
            _ => ParserType::DerivRec,
        }
    }

    /// mapping selected parser to its concrete (untraced) parsing function.
    /// Every non-traced call site should go through this rather than re-matching ParserType itself.
    pub fn parser(&self) -> fn(&str, &crate::types::Regex) -> Option<crate::types::ParseTree> {
        match self {
            ParserType::DerivRec  => crate::parsers::parse_recursive,
            ParserType::DerivLoop => crate::parsers::parse_loop,
            ParserType::DerivBC   => crate::parsers::parse_bitcoded,
            // Both names resolve to the same bit-coded construction -- see
            // the ParserType::PDeriv doc comment.
            ParserType::PDeriv    => crate::parsers::parse_pderiv_bc,
            ParserType::PDerivBC  => crate::parsers::parse_pderiv_bc,
        }
    }
}
