//! Plain-Regex partial-derivative parser (Greedy)
//! Non-bitcoded counterpart of `parsers::bitcoded::pderiv_bc`

pub(crate) mod inject;
pub(crate) mod pderiv_tree;
pub mod parse;

pub use parse::{parse_pderiv_std, parse_pderiv_standard_traced};
pub(crate) use pderiv_tree::pderiv_tree;
