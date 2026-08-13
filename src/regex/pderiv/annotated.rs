//! regex-engine/src/regex/pderiv/annotated.rs
//!
//! Bit-coded Antimirov partial derivatives (pDerivBC), 
//! https://sulzmann.github.io/ProgrammingParadigms/pp-regular-expressions.html#(9)

use std::collections::HashSet;
use crate::types::Regex;
use crate::regex::nullable::standard::nullable;
use crate::regex::simplify::standard::smart_seq;

/// mkEpsBC for plain Regex the pderiv analogue of `parsers::bitcoded::mk_eps_bc`, 
pub fn mk_eps_bits(r: &Regex) -> Vec<bool> {
    match r {
        Regex::Phi => panic!("mk_eps_bits called on Phi"),
        Regex::Lit(c) => panic!("mk_eps_bits called on Lit('{}')", c),

        Regex::Eps => Vec::new(),

        Regex::Alt(r1, r2) => {
            if nullable(r1) {
                let mut bits = vec![false];
                bits.extend(mk_eps_bits(r1));
                bits
            } else {
                let mut bits = vec![true];
                bits.extend(mk_eps_bits(r2));
                bits
            }
        }

        Regex::Seq(r1, r2) => {
            let mut bits = mk_eps_bits(r1);
            bits.extend(mk_eps_bits(r2));
            bits
        }

        Regex::Star(_) => vec![true],
    }
}

/// `nubBy (\(r,_) (s,_) -> r == s)`: stable dedup keeping the *first*
/// occurrence of each residual regex and dropping later duplicates.
fn nub2(pairs: Vec<(Regex, Vec<bool>)>) -> Vec<(Regex, Vec<bool>)> {
    let mut seen: HashSet<Regex> = HashSet::with_capacity(pairs.len());
    let mut out = Vec::with_capacity(pairs.len());
    for (r, bs) in pairs {
        if seen.insert(r.clone()) {
            out.push((r, bs));
        }
    }
    out
}

/// Bit-coded partial derivative: `pDerivBC x r`.
///
/// Returns one `(residual, bits)` pair per surviving strand of nondeterminism. 
/// `bits` records, for that strand alone, every choice (which `Alt` branch, which `Star` iteration boundary) taken to reach
pub fn pderiv_bc(r: &Regex, x: char) -> Vec<(Regex, Vec<bool>)> {
    match r {
        Regex::Eps => Vec::new(),
        Regex::Phi => Vec::new(),

        Regex::Lit(y) => {
            if *y == x {
                vec![(Regex::Eps, Vec::new())]
            } else {
                Vec::new()
            }
        }

        Regex::Alt(r1, r2) => {
            let mut out: Vec<(Regex, Vec<bool>)> = pderiv_bc(r1, x)
                .into_iter()
                .map(|(r1_, mut bs)| {
                    let mut b = vec![false];
                    b.append(&mut bs);
                    (r1_, b)
                })
                .collect();
            out.extend(
                pderiv_bc(r2, x)
                    .into_iter()
                    .map(|(r2_, mut bs)| {
                        let mut b = vec![true];
                        b.append(&mut bs);
                        (r2_, b)
                    }),
            );
            nub2(out)
        }

        Regex::Seq(r1, r2) => {
            if nullable(r1) {
                // Strand 1: continue r1, r2 untouched -- NOT smart-constructed
                // (matches the reference literally
                let mut out: Vec<(Regex, Vec<bool>)> = pderiv_bc(r1, x)
                    .into_iter()
                    .map(|(r1_, bs)| (Regex::seq(r1_, *r2.clone()), bs))
                    .collect();

                // Strand 2: r1 already matched empty (mk_eps_bits(r1) supplies
                // that un-derived prefix), jump straight into r2 
                // mirrors inject's Right case / deriv_bc's fuse(mkEpsBC(r1), ...).
                let eps_bits = mk_eps_bits(r1);
                out.extend(pderiv_bc(r2, x).into_iter().map(|(r2_, bs)| {
                    let mut b = eps_bits.clone();
                    b.extend(bs);
                    (r2_, b)
                }));

                nub2(out)
            } else {
                pderiv_bc(r1, x)
                    .into_iter()
                    .map(|(r1_, bs)| (smart_seq(r1_, r2), bs))
                    .collect()
            }
        }

        Regex::Star(r1) => {
            let out: Vec<(Regex, Vec<bool>)> = pderiv_bc(r1, x)
                .into_iter()
                .map(|(r1_, mut bs)| {
                    let mut b = vec![false];
                    b.append(&mut bs);
                    (smart_seq(r1_, &Regex::star(*r1.clone())), b)
                })
                .collect();
            nub2(out)
        }
    }
}


// -------------------------------
// Tests for pderiv_bc and mk_eps_bits
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Regex;

    fn set(pairs: &[(Regex, Vec<bool>)]) -> HashSet<(Regex, Vec<bool>)> {
        pairs.iter().cloned().collect()
    }

    fn as_set(pairs: Vec<(Regex, Vec<bool>)>) -> HashSet<(Regex, Vec<bool>)> {
        pairs.into_iter().collect()
    }

    // pderiv_bc(Phi/Eps, _) = []
    #[test]
    fn pderiv_bc_phi_empty() {
        assert_eq!(pderiv_bc(&Regex::Phi, 'a'), vec![]);
    }
    #[test]
    fn pderiv_bc_eps_empty() {
        assert_eq!(pderiv_bc(&Regex::Eps, 'a'), vec![]);
    }

    // pderiv_bc(Lit(c), c) = [(Eps, [])]
    #[test]
    fn pderiv_bc_lit_match() {
        assert_eq!(pderiv_bc(&Regex::lit('a'), 'a'), vec![(Regex::Eps, vec![])]);
    }
    #[test]
    fn pderiv_bc_lit_no_match() {
        assert_eq!(pderiv_bc(&Regex::lit('a'), 'b'), vec![]);
    }

    // pderiv_bc(a + ab, 'a') = [(Eps,[false]), (b,[true])]  (worked example, ch.3-style)
    #[test]
    fn pderiv_bc_alt_bits_tag_branch() {
        let r = Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b')));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(
            as_set(d),
            set(&[(Regex::Eps, vec![false]), (Regex::lit('b'), vec![true])])
        );
    }

    // pderiv_bc(Seq(r1,r2), c), r1 not nullable: bits pass through unchanged
    #[test]
    fn pderiv_bc_seq_non_nullable_no_extra_bits() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::lit('b'), vec![])]);
    }

    // pderiv_bc(Seq(r1,r2), c), r1 nullable: both strands present
    #[test]
    fn pderiv_bc_seq_nullable_left_both_strands() {
        // (eps + a) . b  on 'a':
        //   strand 1 (continue r1=eps+a): pderiv_bc(eps+a,'a') = [(Eps,[true])]
        //     -> Cat(Eps, b), bits [true]
        //   strand 2 (r1 matched empty, jump to r2=b): pderiv_bc(b,'a') = [] (b != a)
        let r1 = Regex::alt(Regex::Eps, Regex::lit('a'));
        let r = Regex::seq(r1, Regex::lit('b'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::seq(Regex::Eps, Regex::lit('b')), vec![true])]);
    }

    // pderiv_bc(Star(r), c) tags with a leading `false` (one more iteration)
    #[test]
    fn pderiv_bc_star_tags_iteration() {
        let r = Regex::star(Regex::lit('a'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::star(Regex::lit('a')), vec![false])]);
    }
    #[test]
    fn pderiv_bc_star_wrong_char_empty() {
        let r = Regex::star(Regex::lit('a'));
        assert_eq!(pderiv_bc(&r, 'b'), vec![]);
    }

    // nub2 keeps the first occurrence, dropping a later duplicate's bits
    #[test]
    fn pderiv_bc_alt_dedups_identical_residuals_keeping_first() {
        // (a + a) on 'a': both branches produce Eps; left [false] must win
        let r = Regex::alt(Regex::lit('a'), Regex::lit('a'));
        let d = pderiv_bc(&r, 'a');
        assert_eq!(d, vec![(Regex::Eps, vec![false])]);
    }

    // mk_eps_bits mirrors mk_eps_bc's structure exactly, on plain Regex
    #[test]
    fn mk_eps_bits_eps_is_empty() {
        assert_eq!(mk_eps_bits(&Regex::Eps), Vec::<bool>::new());
    }
    #[test]
    fn mk_eps_bits_star_is_true() {
        assert_eq!(mk_eps_bits(&Regex::star(Regex::lit('a'))), vec![true]);
    }
    #[test]
    fn mk_eps_bits_alt_prefers_left_when_nullable() {
        let r = Regex::alt(Regex::Eps, Regex::lit('a'));
        assert_eq!(mk_eps_bits(&r), vec![false]);
    }
    #[test]
    fn mk_eps_bits_alt_uses_right_when_left_not_nullable() {
        let r = Regex::alt(Regex::lit('a'), Regex::Eps);
        assert_eq!(mk_eps_bits(&r), vec![true]);
    }
    #[test]
    fn mk_eps_bits_seq_concatenates() {
        let r = Regex::seq(
            Regex::alt(Regex::Eps, Regex::lit('a')),
            Regex::star(Regex::lit('b')),
        );
        // left alt nullable via Eps -> [false]; star -> [true]
        assert_eq!(mk_eps_bits(&r), vec![false, true]);
    }
    #[test]
    #[should_panic(expected = "mk_eps_bits called on Phi")]
    fn mk_eps_bits_phi_panics() {
        mk_eps_bits(&Regex::Phi);
    }
    #[test]
    #[should_panic(expected = "mk_eps_bits called on Lit")]
    fn mk_eps_bits_lit_panics() {
        mk_eps_bits(&Regex::lit('a'));
    }
}
