//! regex-engine/src/posix/bitcoded/mk_eps_bc.rs
//! 
//! Bit-coded mkEps (ARegex -> bits)
//! Extracts bit sequence from nullable ARegex

use crate::types::ARegex;
use crate::regex::nullable::annotated::nullable_bc;

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
            let mut result = bs.clone(); // [false, false, false]
            result.push(true);                      // [true] = end of star iterations
            result                                  // [false, false, false, true] 
        }
    }
}


// -------------------------------
// Tests for mk_eps_bc
// -------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ARegex;

    #[test]
    fn mk_eps_bc_eps_returns_its_bits() {
        let result: Vec<bool> = mk_eps_bc(&ARegex::Eps(vec![]));
        assert_eq!(result, Vec::<bool>::new());

        let result2: Vec<bool> = mk_eps_bc(&ARegex::Eps(vec![false, true]));
        assert_eq!(result2, vec![false, true]);
    }

    #[test]
    fn mk_eps_bc_star_appends_true() {
        let ri = ARegex::Star(vec![], Box::new(ARegex::lit('a')));
        let result: Vec<bool> = mk_eps_bc(&ri);
        assert_eq!(result, vec![true]);
    }

    #[test]
    fn mk_eps_bc_star_with_prefix_bits() {
        let ri = ARegex::Star(vec![false, false], Box::new(ARegex::lit('a')));
        let result: Vec<bool> = mk_eps_bc(&ri);
        assert_eq!(result, vec![false, false, true]);
    }

    #[test]
    fn mk_eps_bc_seq_concatenates() {
        // Seq([], Eps([false]), Eps([true])) → [] ++ [false] ++ [true] = [false, true]
        let ri = ARegex::Seq(
            vec![],
            Box::new(ARegex::Eps(vec![false])),
            Box::new(ARegex::Eps(vec![true])),
        );
        let result: Vec<bool> = mk_eps_bc(&ri);
        assert_eq!(result, vec![false, true]);
    }

    #[test]
    fn mk_eps_bc_seq_with_outer_bits() {
        // Seq([true], Eps([]), Star([], _)) → [true] ++ [] ++ [true] = [true, true]
        let ri = ARegex::Seq(
            vec![true],
            Box::new(ARegex::Eps(vec![])),
            Box::new(ARegex::Star(vec![], Box::new(ARegex::lit('x')))),
        );
        let result: Vec<bool> = mk_eps_bc(&ri);
        assert_eq!(result, vec![true, true]);
    }

    #[test]
    fn mk_eps_bc_alt_prefers_left_when_nullable() {
        // Alt([], Eps([false]), Lit([true],'a')) → [] ++ [false] = [false]
        let ri = ARegex::Alt(
            vec![],
            Box::new(ARegex::Eps(vec![false])),
            Box::new(ARegex::Lit(vec![true], 'a')),
        );
        let result: Vec<bool> = mk_eps_bc(&ri);
        assert_eq!(result, vec![false]);
    }

    #[test]
    fn mk_eps_bc_alt_uses_right_when_left_not_nullable() {
        // Alt([], Lit([false],'a'), Eps([true])) → [] ++ [true] = [true]
        let ri = ARegex::Alt(
            vec![],
            Box::new(ARegex::Lit(vec![false], 'a')),
            Box::new(ARegex::Eps(vec![true])),
        );
        let result: Vec<bool> = mk_eps_bc(&ri);
        assert_eq!(result, vec![true]);
    }

    #[test]
    fn mk_eps_bc_alt_with_outer_bits() {
        // Alt([false], Eps([]), Star([],_)) → [false] ++ [] = [false]
        let ri = ARegex::Alt(
            vec![false],
            Box::new(ARegex::Eps(vec![])),
            Box::new(ARegex::Star(vec![], Box::new(ARegex::lit('a')))),
        );
        let result: Vec<bool> = mk_eps_bc(&ri);
        assert_eq!(result, vec![false]);
    }

    #[test]
    #[should_panic(expected = "mk_eps_bc called on Phi")]
    fn mk_eps_bc_phi_panics() {
        mk_eps_bc(&ARegex::Phi);
    }

    #[test]
    #[should_panic(expected = "mk_eps_bc called on Lit")]
    fn mk_eps_bc_lit_panics() {
        mk_eps_bc(&ARegex::Lit(vec![], 'a'));
    }
}