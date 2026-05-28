//! POSIX disambiguation policy for regular expression parsing

pub mod standard;
pub mod bitcoded;
pub mod parser;
pub mod selection;

// Re-export
pub use parser::{parse_posix, match_posix};
pub use standard::{mk_eps, inject, parse_recursive, parse_loop};
pub use bitcoded::parse_bitcoded;
pub use crate::types::{ParseTree, flatten};
pub use selection::ParserType;