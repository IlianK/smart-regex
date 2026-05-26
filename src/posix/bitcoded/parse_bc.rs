//! parse_bitcoded entry point

use crate::types::{Regex, ParseTree};
use crate::debug_println;
use crate::posix::debug::step_reset;
use super::internalize::internalize;
use super::mk_eps_bc::mk_eps_bc;
use super::simp::simp;
use super::decode::decode;
use crate::derivatives::bitcoded::{nullable_bc, deriv_bc};

fn fmt_annotated(ri: &crate::types::ARegex) -> String {
    format!("{}", ri)
}

pub fn parse_bitcoded(input: &str, r: &Regex) -> Option<ParseTree> {
    step_reset();
    debug_println!("BIT-CODED POSIX");
    debug_println!("\n  Input: \"{}\"", input);
    debug_println!("  Regex: {:?}", r);
    debug_println!("\n[--- Internalize (Figure 5) ---]");
    
    let mut ri = internalize(r);
    debug_println!("  ri₀ = {}", fmt_annotated(&ri));
    
    debug_println!("\n[--- Forward Derivative Pass (ri\\ₗl) ---]");
    for (idx, l) in input.chars().enumerate() {
        debug_println!("\n  Step {}: ri{} \\ₗ '{}'", idx, idx, l);
        debug_println!("    before: {}", fmt_annotated(&ri));
        ri = simp(deriv_bc(ri, l));
        debug_println!("    after:  {}", fmt_annotated(&ri));
    }
    
    debug_println!("\n[--- Nullability Check & mkEpsBC (Figure 5) ---]");
    debug_println!("  nullable? {}", nullable_bc(&ri));
    
    if !nullable_bc(&ri) {
        debug_println!("\n  ✗ Not nullable → no match");
        return None;
    }
    
    let bits = mk_eps_bc(&ri);
    debug_println!("  mkEpsBC(ri{}) = {:?}", input.len(), bits);
    
    debug_println!("\n[--- Decode (Figure 4) ---]");
    let tree = decode(r, &bits);
    debug_println!("  decode(r, {:?}) = {:?}", bits, tree);
    debug_println!("  flattened: \"{}\"", crate::types::flatten(&tree));
    
    Some(tree)
}