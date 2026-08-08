//! regex-engine/src/regex/deriv/annotated.rs
//! 
//! Bit-coded Brzozowski derivative for annotated ARegex
//! Propagates and inserts parse tree information as bits during forward pass

use crate::types::ARegex;
use crate::regex::nullable::annotated::nullable_bc;
use crate::posix::bitcoded::internalize::fuse;
use crate::posix::bitcoded::mk_eps_bc::mk_eps_bc;

/// Bit-coded derivative (paper Figure 5)
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
        // Empty star prefix: bs = []; Inner exp: r = Box<Lit([], 'a')>
        ARegex::Star(bs, r) => {
            let d = deriv_bc(*r.clone(), l);     // ri\'a' = Lit([],'a')\'a' = Eps([])
            let fused = fuse(&[false], d);   // fuse [0] Eps([]) = Eps([false])
            let new_star = ARegex::Star(vec![], r); // []@ri* = []@([]@'a')*
            ARegex::Seq(bs, Box::new(fused), Box::new(new_star))
            // = []@(Eps([false]) · []@([]@'a')*)
        }
    }
}


// -------------------------------
// Tests for deriv_bc
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ARegex;

    // deriv_bc(Phi, _) = Phi
    #[test]
    fn deriv_bc_phi_is_phi() {
        assert_eq!(deriv_bc(ARegex::Phi, 'a'), ARegex::Phi);
    }

    // deriv_bc(Eps(bs), _) = Phi  (Eps has no derivative)
    #[test]
    fn deriv_bc_eps_is_phi() {
        assert_eq!(deriv_bc(ARegex::Eps(vec![]), 'a'), ARegex::Phi);
    }

    // deriv_bc(Lit(bs, c), c) = Eps(bs) - bits are preserved
    #[test]
    fn deriv_bc_lit_match_preserves_bits() {
        let ri = ARegex::Lit(vec![false, true], 'a');
        let d = deriv_bc(ri, 'a');
        assert_eq!(d, ARegex::Eps(vec![false, true]));
    }

    // deriv_bc(Lit(bs, c), d≠c) = Phi
    #[test]
    fn deriv_bc_lit_no_match_is_phi() {
        let ri = ARegex::Lit(vec![false], 'a');
        let d = deriv_bc(ri, 'b');
        assert_eq!(d, ARegex::Phi);
    }

    // deriv_bc(Alt(bs, r1, r2), c) = Alt(bs, deriv_bc(r1,c), deriv_bc(r2,c))
    #[test]
    fn deriv_bc_alt_preserves_outer_bits() {
        let ri = ARegex::Alt(
            vec![true],
            Box::new(ARegex::Lit(vec![false], 'a')),
            Box::new(ARegex::Lit(vec![true], 'b')),
        );
        let d = deriv_bc(ri, 'a');
        // Outer bits [true] should be preserved on the Alt wrapper
        match d {
            ARegex::Alt(ref bs, _, _) => assert_eq!(bs, &[true]),
            other => panic!("expected Alt, got {:?}", other),
        }
    }

    // deriv_bc(Star(bs, r), c) = Seq(bs, fuse([false], deriv_bc(r,c)), Star([],r))
    #[test]
    fn deriv_bc_star_gives_seq() {
        let ri = ARegex::Star(vec![], Box::new(ARegex::Lit(vec![], 'a')));
        let d = deriv_bc(ri, 'a');
        assert!(matches!(d, ARegex::Seq(_, _, _)),
            "Star derivative should produce Seq, got {:?}", d);
    }

    // Bit carry-through: fused [false] appears in the left branch of the Star derivative
    #[test]
    fn deriv_bc_star_left_branch_has_false_bit() {
        let ri = ARegex::Star(vec![], Box::new(ARegex::Lit(vec![], 'a')));
        let d = deriv_bc(ri, 'a');
        if let ARegex::Seq(_, left, _) = d {
            // fuse([false], Eps([])) = Eps([false])
            assert_eq!(*left, ARegex::Eps(vec![false]));
        } else {
            panic!("expected Seq");
        }
    }
}