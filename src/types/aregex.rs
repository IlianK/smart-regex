//! Annotated regular expressions (bit-code annotated)
//!
//! Bit convention: false = 0 (Left / start-of-star), true = 1 (Right / end-of-star)

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ARegex {
    /// Empty language 
    Phi,
    /// bs @ ε
    Eps(Vec<bool>),
    /// bs @ l
    Lit(Vec<bool>, char),
    /// bs @ (ri₁ ⊕ ri₂)
    Alt(Vec<bool>, Box<ARegex>, Box<ARegex>),
    /// bs @ (ri₁ ri₂)
    Seq(Vec<bool>, Box<ARegex>, Box<ARegex>),
    /// bs @ ri*
    Star(Vec<bool>, Box<ARegex>),
}

impl ARegex {
    pub fn eps() -> Self { ARegex::Eps(vec![]) }
    pub fn lit(c: char) -> Self { ARegex::Lit(vec![], c) }
    pub fn alt(r1: ARegex, r2: ARegex) -> Self {
        ARegex::Alt(vec![], Box::new(r1), Box::new(r2))
    }
    pub fn seq(r1: ARegex, r2: ARegex) -> Self {
        ARegex::Seq(vec![], Box::new(r1), Box::new(r2))
    }
    pub fn star(r: ARegex) -> Self {
        ARegex::Star(vec![], Box::new(r))
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

// ============================================================================
// Tests for ARegex
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eps_has_empty_bits() {
        assert_eq!(ARegex::eps(), ARegex::Eps(vec![]));
    }

    #[test]
    fn lit_has_empty_bits() {
        assert_eq!(ARegex::lit('x'), ARegex::Lit(vec![], 'x'));
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
        assert_eq!(format!("{}", ARegex::Eps(vec![])), "[]@ε");
    }

    #[test]
    fn display_lit() {
        assert_eq!(format!("{}", ARegex::Lit(vec![], 'a')), "[]@'a'");
    }

    #[test]
    fn clone_and_eq() {
        let ri = ARegex::seq(ARegex::star(ARegex::lit('a')), ARegex::lit('b'));
        assert_eq!(ri.clone(), ri);
    }
}