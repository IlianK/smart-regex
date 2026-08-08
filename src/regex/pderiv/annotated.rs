//! regex-engine/src/regex/pderiv/annotated.rs
//!
//! Bit-coded Antimirov partial derivatives 

use std::collections::HashSet;
use crate::types::ARegex;

/// Bit-coded partial derivative - returns set of annotated derivatives
pub fn pderiv_bc(_ri: ARegex, _l: char) -> HashSet<ARegex> {
    unimplemented!("Bit-coded partial derivatives - future work")
}

// -------------------------------
// Tests for pderiv_bc
// -------------------------------
