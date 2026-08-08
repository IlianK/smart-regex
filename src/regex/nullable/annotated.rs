//! regex-engine/src/regex/nullable/annotated.rs
//! 
//! Nullability for annotated ARegex

use crate::types::ARegex;

/// Decides whether epsilon is in L(ri)
pub fn nullable_bc(ri: &ARegex) -> bool {
    match ri {
        ARegex::Phi => false,
        ARegex::Eps(_) => true,
        ARegex::Lit(_, _) => false,
        ARegex::Alt(_, r1, r2) => nullable_bc(r1) || nullable_bc(r2),
        ARegex::Seq(_, r1, r2) => nullable_bc(r1) && nullable_bc(r2),
        ARegex::Star(_, _) => true,
    }
}

/// Does expression denote empty language?
pub fn is_phi(ri: &ARegex) -> bool {
    match ri {
        ARegex::Phi => true,
        ARegex::Eps(_) => false,
        ARegex::Lit(_, _) => false,
        ARegex::Alt(_, r1, r2) => is_phi(r1) && is_phi(r2),
        ARegex::Seq(_, r1, r2) => is_phi(r1) || is_phi(r2),
        ARegex::Star(_, _) => false,
    }
}


// -------------------------------
// Tests for nullable_bc and is_phi
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ARegex;
 
    #[test] fn phi_is_phi()         { assert!(is_phi(&ARegex::Phi)); }
    #[test] fn eps_not_phi()        { assert!(!is_phi(&ARegex::Eps(vec![]))); }
    #[test] fn phi_not_nullable_bc(){ assert!(!nullable_bc(&ARegex::Phi)); }
    #[test] fn eps_nullable_bc()    { assert!(nullable_bc(&ARegex::Eps(vec![]))); }
    #[test] fn lit_not_nullable_bc(){ assert!(!nullable_bc(&ARegex::Lit(vec![], 'a'))); }
    #[test] fn star_nullable_bc()   { assert!(nullable_bc(&ARegex::star(ARegex::lit('a')))); }
    #[test] fn alt_left_nullable_bc() {
        let ri = ARegex::Alt(vec![], Box::new(ARegex::eps()), Box::new(ARegex::lit('a')));
        assert!(nullable_bc(&ri));
    }
    #[test] fn alt_neither_bc() {
        let ri = ARegex::Alt(vec![], Box::new(ARegex::lit('a')), Box::new(ARegex::lit('b')));
        assert!(!nullable_bc(&ri));
    }
    #[test] fn seq_both_nullable_bc() {
        let ri = ARegex::Seq(vec![], Box::new(ARegex::eps()), Box::new(ARegex::eps()));
        assert!(nullable_bc(&ri));
    }
}
