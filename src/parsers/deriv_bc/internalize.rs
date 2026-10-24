//! src/parsers/deriv_bc/internalize.rs
//!
//! internalize: Regex -> ARegex. fuse: prepend a bit-code prefix.

use crate::types::{Regex, ARegex, Bits};


/// Prepends `bs` onto `ri`'s own bits, O(bs.len()) via `Bits::prepend`
/// regardless of how much `ri` already carries (see src/types/bits.rs).
pub fn fuse(bs: &[bool], ri: ARegex) -> ARegex {
    if bs.is_empty() { return ri; }
    match ri {
        ARegex::Phi => ARegex::Phi,
        ARegex::Eps(p) => ARegex::Eps(p.prepend(bs)),
        ARegex::Lit(p, c) => ARegex::Lit(p.prepend(bs), c),
        ARegex::Alt(p, r1, r2) => ARegex::Alt(p.prepend(bs), r1, r2),
        ARegex::Seq(p, r1, r2) => ARegex::Seq(p.prepend(bs), r1, r2),
        ARegex::Star(p, r) => ARegex::Star(p.prepend(bs), r),
    }
}

// a* = Star(Lit('a'))
pub fn internalize(r: &Regex) -> ARegex {
    match r {
        Regex::Phi => ARegex::Phi,
        Regex::Eps => ARegex::Eps(Bits::empty()),

        // internalize(Lit('a')) = Lit([], 'a') = []@'a'
        Regex::Lit(c) => ARegex::Lit(Bits::empty(), *c),

        // Fusing [False] to the left branch and [True] to the right branch of an Alt
        Regex::Alt(r1, r2) => {
            let ri1 = fuse(&[false], internalize(r1));
            let ri2 = fuse(&[true], internalize(r2));
            ARegex::Alt(Bits::empty(), Box::new(ri1), Box::new(ri2))
        }
        Regex::Seq(r1, r2) => {
            ARegex::Seq(Bits::empty(), Box::new(internalize(r1)), Box::new(internalize(r2)))
        }
        // internalize(Star(...)) = Star([], Lit([], 'a')) = []@(__)*
        Regex::Star(r1) => {
            ARegex::Star(Bits::empty(), Box::new(internalize(r1)))
        }
    }
}


// Tests for internalize and fuse

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
        assert_eq!(internalize(&Regex::Eps), ARegex::Eps(Bits::empty()));
    }

    #[test]
    fn internalize_lit_gives_empty_bits() {
        assert_eq!(internalize(&Regex::lit('a')), ARegex::Lit(Bits::empty(), 'a'));
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
                assert!(matches!(r1.as_ref(), ARegex::Lit(ref b, 'a') if b.to_vec() == vec![false]));
                // right branch carries [true]
                assert!(matches!(r2.as_ref(), ARegex::Lit(ref b, 'b') if b.to_vec() == vec![true]));
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
        let ri = ARegex::Eps(vec![true].into());
        let result = fuse(&[], ri.clone());
        assert_eq!(result, ri);
    }

    #[test]
    fn fuse_prepends_to_eps() {
        let ri = ARegex::Eps(vec![true].into());
        let result = fuse(&[false], ri);
        assert_eq!(result, ARegex::Eps(vec![false, true].into()));
    }

    #[test]
    fn fuse_prepends_to_lit() {
        let ri = ARegex::Lit(Bits::empty(), 'a');
        let result = fuse(&[false, true], ri);
        assert_eq!(result, ARegex::Lit(vec![false, true].into(), 'a'));
    }

    #[test]
    fn fuse_prepends_to_star() {
        let ri = ARegex::Star(vec![true].into(), Box::new(ARegex::lit('a')));
        let result = fuse(&[false], ri);
        assert!(matches!(result, ARegex::Star(ref bs, _) if bs.to_vec() == vec![false, true]));
    }

    #[test]
    fn fuse_on_phi_stays_phi() {
        let result = fuse(&[false], ARegex::Phi);
        assert_eq!(result, ARegex::Phi);
    }

}
