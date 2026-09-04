//! Bit-coded parsers: deriv_bc (POSIX) and pderiv_bc (Greedy), sharing decode.

pub mod decode;
pub mod deriv_bc;
pub mod pderiv_bc;

pub use decode::decode;
pub use deriv_bc::{internalize, fuse, mk_eps_bc};
pub use deriv_bc::{parse_bitcoded, parse_bitcoded_recursive, parse_bitcoded_loop, parse_bitcoded_traced};
pub use pderiv_bc::{parse_pderiv_bc, parse_pderiv_bc_traced};
