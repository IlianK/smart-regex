//! regex-engine/src/posix/bitcoded/mod.rs
//!
//! Bit-coded incremental POSIX parsing

pub mod deriv;
pub mod pderiv;

pub use deriv::{internalize, fuse};
pub use deriv::mk_eps_bc;
pub use deriv::decode;
pub use deriv::{parse_bitcoded, parse_bitcoded_recursive, parse_bitcoded_loop, parse_bitcoded_traced};

pub use pderiv::{parse_pderiv_bc, parse_pderiv_bc_traced};

pub use crate::regex::nullable::annotated::{nullable_bc, is_phi};
pub use crate::regex::deriv::annotated::deriv_bc;
pub use crate::regex::simplify::annotated::simp;
