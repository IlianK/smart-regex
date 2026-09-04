//! regex-engine/src/posix/selection.rs
//!
//!  Parser selection logic shared between library and CLI

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

    pub fn display_name(&self) -> &'static str {
        match self {
            ParserType::DerivStdRec     => "DERIV_STD_REC",
            ParserType::DerivStdLoop    => "DERIV_STD_LOOP",
            ParserType::DerivBc         => "DERIV_BC",
            ParserType::PDerivStd       => "PDERIV_STD",
            ParserType::PDerivBc        => "PDERIV_BC",
        }
    }

    pub fn from_env() -> Vec<ParserType> {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("deriv_std_rec")         => vec![ParserType::DerivStdRec],
            Ok("deriv_std_loop")        => vec![ParserType::DerivStdLoop],
            Ok("deriv_bc")              => vec![ParserType::DerivBc],
            Ok("pderiv_std")            => vec![ParserType::PDerivStd],
            Ok("pderiv_bc")             => vec![ParserType::PDerivBc],
            Ok("all") => vec![
                ParserType::DerivStdRec, 
                ParserType::DerivStdLoop, 
                ParserType::DerivBc],
            _                           => vec![ParserType::DerivStdRec],
        }
    }

    pub fn single_from_env() -> ParserType {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("deriv_std_loop")        => ParserType::DerivStdLoop,
            Ok("deriv_bc")              => ParserType::DerivBc,
            Ok("pderiv_std")            => ParserType::PDerivStd,
            Ok("pderiv_bc")             => ParserType::PDerivBc,
            _                           => ParserType::DerivStdRec,
        }
    }

    /// Mapping selected parser to (untraced) parsing function
    pub fn parser(&self) -> fn(&str, &crate::types::Regex) -> Option<crate::types::ParseTree> {
        match self {
            ParserType::DerivStdRec     => crate::parsers::parse_recursive,
            ParserType::DerivStdLoop    => crate::parsers::parse_loop,
            ParserType::DerivBc         => crate::parsers::parse_bitcoded,
            ParserType::PDerivStd       => crate::parsers::parse_pderiv_std,
            ParserType::PDerivBc        => crate::parsers::parse_pderiv_bc,
        }
    }
}
