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
            ParserType::DerivStdRec     => crate::parsers::parse_deriv_std_rec,
            ParserType::DerivStdLoop    => crate::parsers::parse_deriv_std_loop,
            ParserType::DerivBc         => crate::parsers::parse_deriv_bc,
            ParserType::PDerivStd       => crate::parsers::parse_pderiv_std,
            ParserType::PDerivBc        => crate::parsers::parse_pderiv_bc,
        }
    }
}
