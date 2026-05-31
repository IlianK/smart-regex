//! regex-engine/src/regex/brzozowski/standard.rs
//! 
//! Brzozowski derivative for standard Regex

use crate::types::Regex;
use crate::regex::nullable::standard::nullable;

/// Computes derivative of r based on character x
pub fn deriv(r: &Regex, x: char) -> Regex {
    match r {
        Regex::Phi => Regex::Phi,
        Regex::Eps => Regex::Phi,
        Regex::Lit(c) => {
            if *c == x { Regex::Eps } else { Regex::Phi }
        }
        Regex::Alt(r1, r2) => {
            Regex::alt(deriv(r1, x), deriv(r2, x))
        }
        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                let dr1 = Regex::seq(deriv(r1, x), *r2.clone());
                Regex::alt(dr1, deriv(r2, x))
            } else {
                Regex::seq(deriv(r1, x), *r2.clone())
            }
        }
        Regex::Star(r1) => {
            Regex::seq(deriv(r1, x), Regex::star(*r1.clone()))
        }
    }
}


// ============================================================================
// Tests for deriv
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    // deriv(Phi, _) = Phi
    #[test]
    fn deriv_phi_is_phi() {
        assert_eq!(deriv(&Regex::Phi, 'a'), Regex::Phi);
    }

    // deriv(Eps, _) = Phi
    #[test]
    fn deriv_eps_is_phi() {
        assert_eq!(deriv(&Regex::Eps, 'a'), Regex::Phi);
    }

    // deriv(Lit(c), c) = Eps
    #[test]
    fn deriv_lit_matching_char_gives_eps() {
        assert_eq!(deriv(&Regex::lit('a'), 'a'), Regex::Eps);
    }

    // deriv(Lit(c), d≠c) = Phi
    #[test]
    fn deriv_lit_wrong_char_gives_phi() {
        assert_eq!(deriv(&Regex::lit('a'), 'b'), Regex::Phi);
    }

    // deriv(Alt(r,s), c) = Alt(deriv(r,c), deriv(s,c))
    #[test]
    fn deriv_alt_distributes() {
        let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
        let d = deriv(&r, 'a');
        // Alt(Eps, Phi) - left matched, right didn't
        assert_eq!(d, Regex::alt(Regex::Eps, Regex::Phi));
    }

    // deriv(Seq(r1,r2), c) when r1 NOT nullable = Seq(deriv(r1,c), r2)
    #[test]
    fn deriv_seq_non_nullable_left() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let d = deriv(&r, 'a');
        // Seq(Eps, Lit('b'))
        assert_eq!(d, Regex::seq(Regex::Eps, Regex::lit('b')));
    }

    // deriv(Seq(r1,r2), c) when r1 IS nullable = Alt(Seq(deriv(r1,c),r2), deriv(r2,c))
    #[test]
    fn deriv_seq_nullable_left() {
        // Star(Lit('a')) · Lit('b')
        let r = Regex::seq(Regex::star(Regex::lit('a')), Regex::lit('b'));
        let d = deriv(&r, 'b');
        // r1=Star(a) is nullable, so d = Alt(Seq(deriv(Star(a),'b'), Lit('b')), deriv(Lit('b'),'b'))
        //                             = Alt(Seq(Phi-ish, Lit('b')), Eps)
        // Whatever the exact form, it should be nullable (since "b" matches the full regex)
        use crate::regex::nullable::standard::nullable;
        assert!(nullable(&d), "derivative of nullable-left seq should be nullable on 'b'");
    }

    // deriv(Star(r), c) = Seq(deriv(r,c), Star(r))
    #[test]
    fn deriv_star_unfolds() {
        let r = Regex::star(Regex::lit('a'));
        let d = deriv(&r, 'a');
        // Seq(Eps, Star(Lit('a')))
        assert_eq!(d, Regex::seq(Regex::Eps, Regex::star(Regex::lit('a'))));
    }

    #[test]
    fn deriv_star_wrong_char_gives_phi_seq() {
        let r = Regex::star(Regex::lit('a'));
        let d = deriv(&r, 'b');
        // Seq(Phi, Star(Lit('a'))) - not nullable
        use crate::regex::nullable::standard::nullable;
        assert!(!nullable(&d));
    }
}