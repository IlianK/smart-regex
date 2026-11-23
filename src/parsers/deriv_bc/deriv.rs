//! src/parsers/deriv_bc/deriv.rs
//! 
//! Brzozowski derivative 
//! ARegex: threads parse-tree bits through the forward pass

use std::rc::Rc;

use crate::types::{ARegex, Bits};
use crate::parsers::deriv_bc::nullable::nullable_bc;
use crate::parsers::deriv_bc::internalize::fuse;
use crate::parsers::deriv_bc::mk_eps_bc::mk_eps_bc;

/// Bit-coded derivative (paper Figure 5).
///
/// Takes `&ARegex`, not `ARegex`, matching `nullable_bc`/`mk_eps_bc`'s
/// own convention: every recursive call here only needs to *read* a
/// child to derive it, never to consume/move it, so there is nothing
/// to clone at a call site like `deriv_bc(r1, l)` -- `&Rc<ARegex>`
/// coerces to `&ARegex` for free. The one real cost in this function
/// is wherever a child needs to appear *twice* in the result (Seq's
/// `r2` surviving unchanged alongside its own derivative, Star's `r`
/// likewise) -- there, `r2.clone()`/`r.clone()` is an `Rc::clone`
/// (O(1)), not a subtree-sized copy.
pub fn deriv_bc(ri: &ARegex, l: char) -> ARegex {
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(_) => ARegex::Phi,
        ARegex::Lit(bs, c) => {
            if *c == l {
                ARegex::Eps(bs.clone())
            } else {
                ARegex::Phi
            }
        }
        ARegex::Alt(bs, r1, r2) => {
            let d1 = deriv_bc(r1, l);
            let d2 = deriv_bc(r2, l);
            ARegex::Alt(bs.clone(), Rc::new(d1), Rc::new(d2))
        }
        ARegex::Seq(bs, r1, r2) => {
            if nullable_bc(r1) {
                let eps_bits = mk_eps_bc(r1);
                let d1 = deriv_bc(r1, l);
                let d2 = deriv_bc(r2, l);
                let left_branch = ARegex::Seq(Bits::empty(), Rc::new(d1), r2.clone());
                let right_branch = fuse(&eps_bits, d2);
                ARegex::Alt(bs.clone(), Rc::new(left_branch), Rc::new(right_branch))
            } else {
                let d1 = deriv_bc(r1, l);
                ARegex::Seq(bs.clone(), Rc::new(d1), r2.clone())
            }
        }
        // Empty star prefix: bs = []; Inner exp: r = Rc<Lit([], 'a')>
        ARegex::Star(bs, r) => {
            let d = deriv_bc(r, l);     // ri\'a' = Lit([],'a')\'a' = Eps([])
            let fused = fuse(&[false], d);   // fuse [0] Eps([]) = Eps([false])
            let new_star = ARegex::Star(Bits::empty(), r.clone()); // []@ri* = []@([]@'a')*
            ARegex::Seq(bs.clone(), Rc::new(fused), Rc::new(new_star))
            // = []@(Eps([false]) · []@([]@'a')*)
        }
    }
}


// Tests for deriv_bc

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ARegex;

    // deriv_bc(Phi, _) = Phi
    #[test]
    fn deriv_bc_phi_is_phi() {
        assert_eq!(deriv_bc(&ARegex::Phi, 'a'), ARegex::Phi);
    }

    // deriv_bc(Eps(bs), _) = Phi  (Eps has no derivative)
    #[test]
    fn deriv_bc_eps_is_phi() {
        assert_eq!(deriv_bc(&ARegex::Eps(vec![].into()), 'a'), ARegex::Phi);
    }

    // deriv_bc(Lit(bs, c), c) = Eps(bs) - bits are preserved
    #[test]
    fn deriv_bc_lit_match_preserves_bits() {
        let ri = ARegex::Lit(vec![false, true].into(), 'a');
        let d = deriv_bc(&ri, 'a');
        assert_eq!(d, ARegex::Eps(vec![false, true].into()));
    }

    // deriv_bc(Lit(bs, c), d≠c) = Phi
    #[test]
    fn deriv_bc_lit_no_match_is_phi() {
        let ri = ARegex::Lit(vec![false].into(), 'a');
        let d = deriv_bc(&ri, 'b');
        assert_eq!(d, ARegex::Phi);
    }

    // deriv_bc(Alt(bs, r1, r2), c) = Alt(bs, deriv_bc(r1,c), deriv_bc(r2,c))
    #[test]
    fn deriv_bc_alt_preserves_outer_bits() {
        let ri = ARegex::Alt(
            vec![true].into(),
            Rc::new(ARegex::Lit(vec![false].into(), 'a')),
            Rc::new(ARegex::Lit(vec![true].into(), 'b')),
        );
        let d = deriv_bc(&ri, 'a');
        // Outer bits [true] should be preserved on the Alt wrapper
        match d {
            ARegex::Alt(ref bs, _, _) => assert_eq!(bs.to_vec(), vec![true]),
            other => panic!("expected Alt, got {:?}", other),
        }
    }

    // deriv_bc(Star(bs, r), c) = Seq(bs, fuse([false], deriv_bc(r,c)), Star([],r))
    #[test]
    fn deriv_bc_star_gives_seq() {
        let ri = ARegex::Star(vec![].into(), Rc::new(ARegex::Lit(vec![].into(), 'a')));
        let d = deriv_bc(&ri, 'a');
        assert!(matches!(d, ARegex::Seq(_, _, _)),
            "Star derivative should produce Seq, got {:?}", d);
    }

    // Bit carry-through: fused [false] appears in the left branch of the Star derivative
    #[test]
    fn deriv_bc_star_left_branch_has_false_bit() {
        let ri = ARegex::Star(vec![].into(), Rc::new(ARegex::Lit(vec![].into(), 'a')));
        let d = deriv_bc(&ri, 'a');
        if let ARegex::Seq(_, left, _) = d {
            // fuse([false], Eps([])) = Eps([false])
            assert_eq!(*left, ARegex::Eps(vec![false].into()));
        } else {
            panic!("expected Seq");
        }
    }
}