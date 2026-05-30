//! Regular expression data type definition

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Regex {
    /// Empty language: L(Phi) = {}
    Phi,

    /// Empty word: L(Eps) = {epsilon}
    Eps,

    /// Single character: L(Lit('a')) = {"a"}
    Lit(char),

    /// Sequence: L(Seq(r,s)) = { v++w | v in L(r), w in L(s) }
    Seq(Box<Regex>, Box<Regex>),

    /// Alternative: L(Alt(r,s)) = L(r) union L(s)
    Alt(Box<Regex>, Box<Regex>),
    
    /// Kleene star: L(Star(r)) = {epsilon} union L(r)·L(Star(r))
    Star(Box<Regex>),
}

impl Regex {
    pub fn seq(r: Regex, s: Regex) -> Regex {
        Regex::Seq(Box::new(r), Box::new(s))
    }
    
    pub fn alt(r: Regex, s: Regex) -> Regex {
        Regex::Alt(Box::new(r), Box::new(s))
    }
    
    pub fn star(r: Regex) -> Regex {
        Regex::Star(Box::new(r))
    }
    
    pub fn lit(c: char) -> Regex {
        Regex::Lit(c)
    }
}

// ============================================================================
// Tests for Regex
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_constructor() {
        assert_eq!(Regex::lit('a'), Regex::Lit('a'));
    }

    #[test]
    fn seq_constructor() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        assert_eq!(r, Regex::Seq(Box::new(Regex::Lit('a')), Box::new(Regex::Lit('b'))));
    }

    #[test]
    fn alt_constructor() {
        let r = Regex::alt(Regex::lit('a'), Regex::Eps);
        assert_eq!(r, Regex::Alt(Box::new(Regex::Lit('a')), Box::new(Regex::Eps)));
    }

    #[test]
    fn star_constructor() {
        let r = Regex::star(Regex::lit('a'));
        assert_eq!(r, Regex::Star(Box::new(Regex::Lit('a'))));
    }

    #[test]
    fn clone_and_eq() {
        let r = Regex::seq(Regex::star(Regex::lit('a')), Regex::lit('b'));
        assert_eq!(r.clone(), r);
    }

    #[test]
    fn phi_and_eps_are_distinct() {
        assert_ne!(Regex::Phi, Regex::Eps);
    }
}