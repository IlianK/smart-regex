//! Partial-derivative parser, plain Regex + injection closures (Greedy); non-bitcoded counterpart of pderiv_bc.

pub(crate) mod inject;
pub(crate) mod pderiv_tree;
pub mod parse;
pub mod traced;

pub use parse::parse_pderiv_std;
pub use traced::parse_pderiv_std_traced;
pub(crate) use pderiv_tree::pderiv_tree;
