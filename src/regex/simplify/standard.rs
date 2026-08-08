//! regex-engine/src/regex/simplify/standard.rs
//! 
//! Simplification for standard Regex

use crate::types::Regex;

/// Simplify regular expression using algebraic laws
pub fn simplify(r: Regex) -> Regex {
    match r {
        Regex::Seq(r, s) => {
            let r = simplify(*r);
            let s = simplify(*s);
            match (&r, &s) {
                (Regex::Phi, _) => Regex::Phi,
                (_, Regex::Phi) => Regex::Phi,
                (Regex::Eps, _) => s,
                (_, Regex::Eps) => r,
                _ => Regex::seq(r, s),
            }
        }
        Regex::Alt(r, s) => {
            let r = simplify(*r);
            let s = simplify(*s);
            match (&r, &s) {
                (Regex::Phi, _) => s,
                (_, Regex::Phi) => r,
                _ if r == s => r,
                _ => Regex::alt(r, s),
            }
        }
        Regex::Star(r) => {
            let r = simplify(*r);
            match r {
                Regex::Eps => Regex::Eps,
                Regex::Phi => Regex::Eps,
                r => Regex::star(r),
            }
        }
        other => other,
    }
}

/// Smart constructor: normalizes Eps . r to r
pub fn smart_seq(r: Regex, s: &Regex) -> Regex {
    match r {
        Regex::Eps => s.clone(),
        r => Regex::seq(r, s.clone()),
    }
}


// -------------------------------
// Tests for simplify and smart_seq
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;
 
    // simplify
    #[test] fn phi_seq_r_is_phi() {
        assert_eq!(simplify(Regex::seq(Regex::Phi, Regex::lit('a'))), Regex::Phi);
    }
    #[test] fn r_seq_phi_is_phi() {
        assert_eq!(simplify(Regex::seq(Regex::lit('a'), Regex::Phi)), Regex::Phi);
    }
    #[test] fn eps_seq_r_is_r() {
        assert_eq!(simplify(Regex::seq(Regex::Eps, Regex::lit('a'))), Regex::lit('a'));
    }
    #[test] fn r_seq_eps_is_r() {
        assert_eq!(simplify(Regex::seq(Regex::lit('a'), Regex::Eps)), Regex::lit('a'));
    }
    #[test] fn phi_alt_r_is_r() {
        assert_eq!(simplify(Regex::alt(Regex::Phi, Regex::lit('b'))), Regex::lit('b'));
    }
    #[test] fn r_alt_phi_is_r() {
        assert_eq!(simplify(Regex::alt(Regex::lit('b'), Regex::Phi)), Regex::lit('b'));
    }
    #[test] fn r_alt_r_is_r() {
        assert_eq!(simplify(Regex::alt(Regex::lit('a'), Regex::lit('a'))), Regex::lit('a'));
    }
    #[test] fn star_eps_is_eps() {
        assert_eq!(simplify(Regex::star(Regex::Eps)), Regex::Eps);
    }
    #[test] fn star_phi_is_eps() {
        assert_eq!(simplify(Regex::star(Regex::Phi)), Regex::Eps);
    }
    #[test] fn lit_unchanged() {
        assert_eq!(simplify(Regex::lit('x')), Regex::lit('x'));
    }
 
    // smart_seq
    #[test] fn smart_seq_eps_left_is_identity() {
        let r = Regex::lit('a');
        assert_eq!(smart_seq(Regex::Eps, &r), r);
    }
    #[test] fn smart_seq_non_eps_wraps() {
        let result = smart_seq(Regex::lit('a'), &Regex::lit('b'));
        assert_eq!(result, Regex::seq(Regex::lit('a'), Regex::lit('b')));
    }
}
