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