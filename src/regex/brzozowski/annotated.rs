//! Bit-coded Brzozowski derivative for annotated ARegex

use crate::types::ARegex;
use crate::regex::nullable::annotated::nullable_bc;
use crate::posix::bitcoded::internalize::fuse;
use crate::posix::bitcoded::mk_eps_bc::mk_eps_bc;

/// Bit-coded derivative following paper Figure 5
pub fn deriv_bc(ri: ARegex, l: char) -> ARegex {
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(_) => ARegex::Phi,
        ARegex::Lit(bs, c) => {
            if c == l {
                ARegex::Eps(bs)
            } else {
                ARegex::Phi
            }
        }
        ARegex::Alt(bs, r1, r2) => {
            let d1 = deriv_bc(*r1, l);
            let d2 = deriv_bc(*r2, l);
            ARegex::Alt(bs, Box::new(d1), Box::new(d2))
        }
        ARegex::Seq(bs, r1, r2) => {
            if nullable_bc(&r1) {
                let eps_bits = mk_eps_bc(&r1);
                let d1 = deriv_bc(*r1, l);
                let d2 = deriv_bc(*r2.clone(), l);
                let left_branch = ARegex::Seq(vec![], Box::new(d1), r2);
                let right_branch = fuse(&eps_bits, d2);
                ARegex::Alt(bs, Box::new(left_branch), Box::new(right_branch))
            } else {
                let d1 = deriv_bc(*r1, l);
                ARegex::Seq(bs, Box::new(d1), r2)
            }
        }
        ARegex::Star(bs, r) => {
            let d = deriv_bc(*r.clone(), l);
            let fused = fuse(&[false], d);
            let new_star = ARegex::Star(vec![], r);
            ARegex::Seq(bs, Box::new(fused), Box::new(new_star))
        }
    }
}