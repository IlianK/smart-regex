//! POSIX disambiguation policy for regular expression parsing
//!
//! Based on: Sulzmann & Lu, "POSIX Regular Expression Parsing with Derivatives"
//! FLOPS 2014

mod parse_tree;
mod mk_eps;
mod inject;
mod parser;

// Re-exports
pub use parse_tree::{ParseTree, flatten};
pub use mk_eps::mk_eps;
pub use inject::inject;
pub use parser::{parse_posix, match_posix};