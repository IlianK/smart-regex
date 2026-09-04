//! regex-engine/src/parsers/mod.rs
//! 
//! Parsers (Greedy/POSIX x Standard/Bitcoded parse tree)

pub mod standard;
pub mod bitcoded;
pub mod parser;
pub mod selection;

// Re-export parsers
pub use parser::{parse};
pub use standard::{mk_eps, inject, parse_recursive, parse_loop, parse_loop_traced, parse_recursive_traced};
pub use standard::{parse_pderiv_std, parse_pderiv_standard_traced};
pub use bitcoded::{parse_bitcoded, parse_bitcoded_traced};
pub use bitcoded::{parse_pderiv_bc, parse_pderiv_bc_traced};

// Re-export types
pub use crate::types::{ParseTree, flatten};
pub use selection::ParserType;