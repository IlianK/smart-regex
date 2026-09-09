//! The four parsers, one flat subfolder each: deriv_std, deriv_bc, pderiv_std, pderiv_bc. See docs/PARSERS.md.

pub mod deriv_std;
pub mod deriv_bc;
pub mod pderiv_std;
pub mod pderiv_bc;
pub mod parser;
pub mod selection;

// Re-export parsers
pub use parser::parse;
pub use deriv_std::{parse_deriv_std_rec, parse_deriv_std_loop, parse_loop_traced, parse_recursive_traced};
pub use pderiv_std::{parse_pderiv_std, parse_pderiv_std_traced};
pub use deriv_bc::{parse_deriv_bc, parse_bitcoded_traced};
pub use pderiv_bc::{parse_pderiv_bc, parse_pderiv_bc_traced};

// Re-export types
pub use crate::types::{ParseTree, flatten};
pub use selection::ParserType;
