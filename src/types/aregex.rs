//! src/types/aregex.rs
//!
//! Annotated regular expressions (bit-code annotated)
//! Bit convention: false = 0 (Left / start-of-star), true = 1 (Right / end-of-star)
//!
//! Recursive fields are `Rc<ARegex>`, not `Box<ARegex>`: `deriv_bc` and
//! `simp` clone whole subexpressions routinely (e.g. `smart_seq`-style
//! collapses, `fuse_bits` rewrapping), and a plain `Box`-based `Clone`
//! walks the entire subtree being cloned. `Rc::clone` is O(1)
//! regardless of subtree size -- the same reasoning `AppendBits`/`Inj`
//! (src/types/bits.rs, src/parsers/pderiv_std/inject.rs) already apply
//! to bit/injection *histories*, just applied to the expression tree
//! itself. Nothing reads or writes through a `&mut ARegex` anywhere in
//! this codebase (confirmed), so there is no in-place-mutation use
//! this loses.

use std::rc::Rc;

use super::Bits;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ARegex {
    /// Empty language
    Phi,
    /// bs @ ε
    Eps(Bits),
    /// bs @ l
    Lit(Bits, char),
    /// bs @ (ri₁ ⊕ ri₂)
    Alt(Bits, Rc<ARegex>, Rc<ARegex>),
    /// bs @ (ri₁ ri₂)
    Seq(Bits, Rc<ARegex>, Rc<ARegex>),
    /// bs @ ri*
    Star(Bits, Rc<ARegex>),
}

impl ARegex {
    pub fn eps() -> Self { ARegex::Eps(Bits::empty()) }
    pub fn lit(c: char) -> Self { ARegex::Lit(Bits::empty(), c) }
    pub fn alt(r1: ARegex, r2: ARegex) -> Self {
        ARegex::Alt(Bits::empty(), Rc::new(r1), Rc::new(r2))
    }
    pub fn seq(r1: ARegex, r2: ARegex) -> Self {
        ARegex::Seq(Bits::empty(), Rc::new(r1), Rc::new(r2))
    }
    pub fn star(r: ARegex) -> Self {
        ARegex::Star(Bits::empty(), Rc::new(r))
    }
}

impl std::fmt::Display for ARegex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ARegex::Phi => write!(f, "φ"),
            ARegex::Eps(bs) => write!(f, "{:?}@ε", bs),
            ARegex::Lit(bs, c) => write!(f, "{:?}@'{}'", bs, c),
            ARegex::Alt(bs, r1, r2) => write!(f, "{:?}@({} ⊕ {})", bs, r1, r2),
            ARegex::Seq(bs, r1, r2) => write!(f, "{:?}@({} · {})", bs, r1, r2),
            ARegex::Star(bs, r) => write!(f, "{:?}@({})*", bs, r),
        }
    }
}

// -------------------------------
// Tests for ARegex
// -------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eps_has_empty_bits() {
        assert_eq!(ARegex::eps(), ARegex::Eps(Bits::empty()));
    }

    #[test]
    fn lit_has_empty_bits() {
        assert_eq!(ARegex::lit('x'), ARegex::Lit(Bits::empty(), 'x'));
    }

    #[test]
    fn alt_constructor_has_empty_outer_bits() {
        let ri = ARegex::alt(ARegex::eps(), ARegex::lit('a'));
        assert!(matches!(ri, ARegex::Alt(ref bs, _, _) if bs.is_empty()));
    }

    #[test]
    fn seq_constructor_has_empty_outer_bits() {
        let ri = ARegex::seq(ARegex::lit('a'), ARegex::lit('b'));
        assert!(matches!(ri, ARegex::Seq(ref bs, _, _) if bs.is_empty()));
    }

    #[test]
    fn star_constructor_has_empty_outer_bits() {
        let ri = ARegex::star(ARegex::lit('a'));
        assert!(matches!(ri, ARegex::Star(ref bs, _) if bs.is_empty()));
    }

    #[test]
    fn display_phi() {
        assert_eq!(format!("{}", ARegex::Phi), "φ");
    }

    #[test]
    fn display_eps() {
        assert_eq!(format!("{}", ARegex::Eps(Bits::empty())), "[]@ε");
    }

    #[test]
    fn display_lit() {
        assert_eq!(format!("{}", ARegex::Lit(Bits::empty(), 'a')), "[]@'a'");
    }

    #[test]
    fn clone_and_eq() {
        let ri = ARegex::seq(ARegex::star(ARegex::lit('a')), ARegex::lit('b'));
        assert_eq!(ri.clone(), ri);
    }
}