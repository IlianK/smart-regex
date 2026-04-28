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