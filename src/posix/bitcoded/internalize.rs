//! internalize: Regex → ARegex
//! fuse: prepend bit-code prefix

use crate::types::{Regex, ARegex};

pub fn fuse(bs: &[bool], ri: ARegex) -> ARegex {
    if bs.is_empty() { return ri; }
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(p) => ARegex::Eps(combine(bs, p)),
        ARegex::Lit(p, c) => ARegex::Lit(combine(bs, p), c),
        ARegex::Alt(p, r1, r2) => ARegex::Alt(combine(bs, p), r1, r2),
        ARegex::Seq(p, r1, r2) => ARegex::Seq(combine(bs, p), r1, r2),
        ARegex::Star(p, r) => ARegex::Star(combine(bs, p), r),
    }
}

fn combine(bs: &[bool], p: Vec<bool>) -> Vec<bool> {
    let mut new = bs.to_vec();
    new.extend(p);
    new
}

pub fn internalize(r: &Regex) -> ARegex {
    match r {
        Regex::Phi => ARegex::Phi,
        Regex::Eps => ARegex::Eps(vec![]),
        Regex::Lit(c) => ARegex::Lit(vec![], *c),
        Regex::Alt(r1, r2) => {
            let ri1 = fuse(&[false], internalize(r1));
            let ri2 = fuse(&[true], internalize(r2));
            ARegex::Alt(vec![], Box::new(ri1), Box::new(ri2))
        }
        Regex::Seq(r1, r2) => {
            ARegex::Seq(vec![], Box::new(internalize(r1)), Box::new(internalize(r2)))
        }
        Regex::Star(r1) => {
            ARegex::Star(vec![], Box::new(internalize(r1)))
        }
    }
}

// ============================================================================
// Tests for internalize and fuse
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Regex, ARegex};
 
    #[test]
    fn internalize_phi_is_phi() {
        assert_eq!(internalize(&Regex::Phi), ARegex::Phi);
    }

    #[test]
    fn internalize_eps_gives_empty_bits() {
        assert_eq!(internalize(&Regex::Eps), ARegex::Eps(vec![]));
    }

    #[test]
    fn internalize_lit_gives_empty_bits() {
        assert_eq!(internalize(&Regex::lit('a')), ARegex::Lit(vec![], 'a'));
    }

    #[test]
    fn internalize_alt_fuses_false_to_left_true_to_right() {
        // internalize(r1 + r2) = []@(([false]@ri1) ⊕ ([true]@ri2))
        let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
        let ri = internalize(&r);
        match ri {
            ARegex::Alt(ref bs, ref r1, ref r2) => {
                assert!(bs.is_empty(), "outer bits should be empty");
                // left branch carries [false]
                assert!(matches!(r1.as_ref(), ARegex::Lit(ref b, 'a') if b == &[false]));
                // right branch carries [true]
                assert!(matches!(r2.as_ref(), ARegex::Lit(ref b, 'b') if b == &[true]));
            }
            other => panic!("expected Alt, got {:?}", other),
        }
    }

    #[test]
    fn internalize_seq_has_empty_outer_bits() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let ri = internalize(&r);
        assert!(matches!(ri, ARegex::Seq(ref bs, _, _) if bs.is_empty()));
    }

    #[test]
    fn internalize_star_has_empty_outer_bits() {
        let r = Regex::star(Regex::lit('a'));
        let ri = internalize(&r);
        assert!(matches!(ri, ARegex::Star(ref bs, _) if bs.is_empty()));
    }

    #[test]
    fn fuse_empty_prefix_is_identity() {
        let ri = ARegex::Eps(vec![true]);
        let result = fuse(&[], ri.clone());
        assert_eq!(result, ri);
    }

    #[test]
    fn fuse_prepends_to_eps() {
        let ri = ARegex::Eps(vec![true]);
        let result = fuse(&[false], ri);
        assert_eq!(result, ARegex::Eps(vec![false, true]));
    }

    #[test]
    fn fuse_prepends_to_lit() {
        let ri = ARegex::Lit(vec![], 'a');
        let result = fuse(&[false, true], ri);
        assert_eq!(result, ARegex::Lit(vec![false, true], 'a'));
    }

    #[test]
    fn fuse_prepends_to_star() {
        let ri = ARegex::Star(vec![true], Box::new(ARegex::lit('a')));
        let result = fuse(&[false], ri);
        assert!(matches!(result, ARegex::Star(ref bs, _) if bs == &[false, true]));
    }

    #[test]
    fn fuse_on_phi_stays_phi() {
        let result = fuse(&[false], ARegex::Phi);
        assert_eq!(result, ARegex::Phi);
    }

}
