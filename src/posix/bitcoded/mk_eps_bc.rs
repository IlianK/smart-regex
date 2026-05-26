//! Bit-coded mkEps (ARegex → bits)

use crate::types::ARegex;
use crate::derivatives::bitcoded::nullable_bc;

pub fn mk_eps_bc(ri: &ARegex) -> Vec<bool> {
    match ri {
        ARegex::Phi => panic!("mk_eps_bc called on Phi"),
        ARegex::Lit(_, c) => panic!("mk_eps_bc called on Lit('{}')", c),
        ARegex::Eps(bs) => bs.clone(),
        ARegex::Alt(bs, r1, r2) => {
            let mut result = bs.clone();
            if nullable_bc(r1) {
                result.extend(mk_eps_bc(r1));
            } else {
                result.extend(mk_eps_bc(r2));
            }
            result
        }
        ARegex::Seq(bs, r1, r2) => {
            let mut result = bs.clone();
            result.extend(mk_eps_bc(r1));
            result.extend(mk_eps_bc(r2));
            result
        }
        ARegex::Star(bs, _) => {
            let mut result = bs.clone();
            result.push(true);
            result
        }
    }
}