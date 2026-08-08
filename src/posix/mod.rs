//! regex-engine/src/posix/mod.rs
//! 
//! POSIX disambiguation policy for regular expression parsing

pub mod standard;
pub mod bitcoded;
pub mod parser;
pub mod selection;

// Re-export parsers
pub use parser::{parse_posix};
pub use standard::{mk_eps, inject, parse_recursive, parse_loop, parse_loop_traced, parse_recursive_traced};
pub use bitcoded::{parse_bitcoded, parse_bitcoded_traced};

// Re-export types
pub use crate::types::{ParseTree, flatten};
pub use selection::ParserType;