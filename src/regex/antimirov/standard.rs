//! regex-engine/src/regex/antimirov/standard.rs
//! 
//! Antimirov partial derivatives for standard Regex

use std::collections::HashSet;
use crate::types::Regex;
use crate::regex::nullable::standard::nullable;
use crate::regex::simplify::standard::smart_seq;

/// Computes set of partial derivatives
pub fn pderiv(r: &Regex, x: char) -> HashSet<Regex> {
    match r {
        Regex::Phi => HashSet::new(),
        Regex::Eps => HashSet::new(),
        Regex::Lit(c) => {
            let mut set = HashSet::new();
            if *c == x { set.insert(Regex::Eps); }
            set
        }
        Regex::Alt(r, s) => {
            let mut set = pderiv(r, x);
            set.extend(pderiv(s, x));
            set
        }
        Regex::Seq(r, s) => {
            let mut set: HashSet<Regex> = pderiv(r, x)
                .into_iter()
                .map(|r_prime| smart_seq(r_prime, s))
                .collect();
            if nullable(r) {
                set.extend(pderiv(s, x));
            }
            set
        }
        Regex::Star(r) => {
            pderiv(r, x)
                .into_iter()
                .map(|r_prime| smart_seq(r_prime, &Regex::star(*r.clone())))
                .collect()
        }
    }
}


// ============================================================================
// Tests for pderiv
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;
    use std::collections::HashSet;

    fn set(rs: Vec<Regex>) -> HashSet<Regex> {
        rs.into_iter().collect()
    }

    // pderiv(Phi, _) = {}
    #[test]
    fn pderiv_phi_empty_set() {
        assert_eq!(pderiv(&Regex::Phi, 'a'), set(vec![]));
    }

    // pderiv(Eps, _) = {}
    #[test]
    fn pderiv_eps_empty_set() {
        assert_eq!(pderiv(&Regex::Eps, 'a'), set(vec![]));
    }

    // pderiv(Lit(c), c) = {Eps}
    #[test]
    fn pderiv_lit_match_gives_eps() {
        assert_eq!(pderiv(&Regex::lit('a'), 'a'), set(vec![Regex::Eps]));
    }

    // pderiv(Lit(c), d≠c) = {}
    #[test]
    fn pderiv_lit_no_match_empty() {
        assert_eq!(pderiv(&Regex::lit('a'), 'b'), set(vec![]));
    }

    // pderiv(Alt(r,s), c) = pderiv(r,c) ∪ pderiv(s,c)
    #[test]
    fn pderiv_alt_is_union() {
        let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
        let d = pderiv(&r, 'a');
        assert_eq!(d, set(vec![Regex::Eps])); // only left branch contributed
    }

    #[test]
    fn pderiv_alt_both_branches_contribute() {
        // Alt(Lit('a'), Lit('a')) - both branches give Eps, but it's a set so still {Eps}
        let r = Regex::alt(Regex::lit('a'), Regex::lit('a'));
        let d = pderiv(&r, 'a');
        assert_eq!(d, set(vec![Regex::Eps]));
    }

    // pderiv(Seq(r1,r2), c) when r1 NOT nullable = {smart_seq(r', r2) | r' ∈ pderiv(r1,c)}
    #[test]
    fn pderiv_seq_non_nullable_left() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let d = pderiv(&r, 'a');
        // pderiv(Lit('a'), 'a') = {Eps}, smart_seq(Eps, Lit('b')) = Lit('b')
        assert_eq!(d, set(vec![Regex::lit('b')]));
    }

    // pderiv(Seq(r1,r2), c) when r1 nullable adds pderiv(r2,c) as well
    #[test]
    fn pderiv_seq_nullable_left_adds_right_derivs() {
        // Seq(Star(Lit('b')), Lit('b'))  - on 'b'
        // pderiv(Star(b), b) = {smart_seq(Eps, Star(b))} = {Star(b)}
        // r1 is nullable so also add pderiv(Lit('b'), 'b') = {Eps}
        let r = Regex::seq(Regex::star(Regex::lit('b')), Regex::lit('b'));
        let d = pderiv(&r, 'b');
        assert!(d.contains(&Regex::Eps),
            "nullable left branch means right deriv (Eps) should be in result");
        assert!(d.contains(&Regex::seq(Regex::star(Regex::lit('b')), Regex::lit('b')))
            || d.contains(&Regex::star(Regex::lit('b'))),
            "left branch derivative should also be present");
    }

    // pderiv(Star(r), c) = {smart_seq(r', Star(r)) | r' ∈ pderiv(r,c)}
    #[test]
    fn pderiv_star_gives_seq_with_star() {
        let r = Regex::star(Regex::lit('a'));
        let d = pderiv(&r, 'a');
        // pderiv(Lit('a'), 'a') = {Eps}, smart_seq(Eps, Star(a)) = Star(a)
        assert_eq!(d, set(vec![Regex::star(Regex::lit('a'))]));
    }

    #[test]
    fn pderiv_star_wrong_char_empty() {
        let r = Regex::star(Regex::lit('a'));
        assert_eq!(pderiv(&r, 'b'), set(vec![]));
    }
}