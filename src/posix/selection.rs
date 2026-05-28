//! Parser selection logic shared between library and CLI

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserType {
    Recursive,
    Loop,
    Bitcoded,
}

impl ParserType {
    pub fn name(&self) -> &'static str {
        match self {
            ParserType::Recursive => "recursive",
            ParserType::Loop => "loop",
            ParserType::Bitcoded => "bitcoded",
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            ParserType::Recursive => "RECURSIVE",
            ParserType::Loop => "LOOP",
            ParserType::Bitcoded => "BITCODED",
        }
    }
    
    pub fn from_env() -> Vec<ParserType> {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("recursive") => vec![ParserType::Recursive],
            Ok("loop") => vec![ParserType::Loop],
            Ok("bitcoded") => vec![ParserType::Bitcoded],
            Ok("all") => vec![ParserType::Recursive, ParserType::Loop, ParserType::Bitcoded],
            _ => vec![ParserType::Recursive],
        }
    }
    
    pub fn single_from_env() -> ParserType {
        match std::env::var("REGEX_PARSER").as_deref() {
            Ok("loop") => ParserType::Loop,
            Ok("bitcoded") => ParserType::Bitcoded,
            _ => ParserType::Recursive,
        }
    }
    
    pub fn parser(&self) -> fn(&str, &crate::types::Regex) -> Option<crate::types::ParseTree> {
        match self {
            ParserType::Recursive => crate::posix::parse_recursive,
            ParserType::Loop => crate::posix::parse_loop,
            ParserType::Bitcoded => crate::posix::parse_bitcoded,
        }
    }
}