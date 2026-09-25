//! Lowers an `ExtPat` (surface syntax) down into the core `Regex`.

use super::alphabet;
use super::ext_pattern::ExtPat;
use crate::types::Regex;

/// Repeating a bound past this many copies is refused 
const MAX_BOUND: u32 = 1000;

/// Lower an `ExtPat` into this crate's core `Regex`.
pub fn translate(ep: &ExtPat) -> Result<Regex, String> {
    match ep {
        ExtPat::Empty => Ok(Regex::Eps),
        ExtPat::Eps => Ok(Regex::Eps),
        ExtPat::Never => Ok(Regex::Phi),
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => translate(inner),
        ExtPat::Or(alts) => {
            let mut iter = alts.iter();
            let mut acc = translate(iter.next().ok_or("empty alternation")?)?;
            for a in iter {
                acc = Regex::alt(acc, translate(a)?);
            }
            Ok(acc)
        }
        ExtPat::Concat(parts) => {
            let mut iter = parts.iter();
            let mut acc = match iter.next() {
                Some(p) => translate(p)?,
                None => return Ok(Regex::Eps),
            };
            for p in iter {
                acc = Regex::seq(acc, translate(p)?);
            }
            Ok(acc)
        }
        ExtPat::Opt(inner, _greedy) => Ok(Regex::alt(translate(inner)?, Regex::Eps)),
        ExtPat::Plus(inner, _greedy) => {
            let r = translate(inner)?;
            Ok(Regex::seq(r.clone(), Regex::star(r)))
        }
        ExtPat::Star(inner, _greedy) => Ok(Regex::star(translate(inner)?)),
        ExtPat::Bound(inner, lo, hi, _greedy) => desugar_bound(translate(inner)?, *lo, *hi),
        ExtPat::Carat | ExtPat::Dollar => Ok(Regex::Eps),
        ExtPat::Dot => alt_of_chars(alphabet::alphabet()),
        ExtPat::Any(cs) => alt_of_chars(cs.clone()),
        ExtPat::NoneOf(cs) => alt_of_chars(alphabet::complement(cs)),
        ExtPat::Escape(c) => translate_escape(*c),
        ExtPat::Char(c) => Ok(Regex::lit(*c)),
        ExtPat::WordBoundary(_) => {
            Err("word boundary '\\b'/'\\B' is position-dependent, not a regular-language \
                 construct -- unsupported".to_string())
        }
    }
}

fn translate_escape(c: char) -> Result<Regex, String> {
    match c {
        'd' => alt_of_chars(alphabet::digit_chars()),
        'D' => alt_of_chars(alphabet::complement(&alphabet::digit_chars())),
        'w' => alt_of_chars(alphabet::word_chars()),
        'W' => alt_of_chars(alphabet::complement(&alphabet::word_chars())),
        's' => alt_of_chars(alphabet::space_chars()),
        'S' => alt_of_chars(alphabet::complement(&alphabet::space_chars())),
        // Any other escaped char (\. \( \) \[ \] \{ \} \| \+ \* \? \^ \$ \\, ...): a literal
        _ => Ok(Regex::lit(c)),
    }
}

/// `(alphabet)*` is "any run of characters"
pub(crate) fn wildcard_run() -> Regex {
    Regex::star(alt_of_chars(alphabet::alphabet()).expect("alphabet() is non-empty"))
}

fn alt_of_chars(mut cs: Vec<char>) -> Result<Regex, String> {
    cs.sort_unstable();
    cs.dedup();
    let mut iter = cs.into_iter();
    let first = iter.next().ok_or("empty character class")?;
    Ok(iter.fold(Regex::lit(first), |acc, c| Regex::alt(acc, Regex::lit(c))))
}

fn desugar_bound(r: Regex, lo: u32, hi: Option<u32>) -> Result<Regex, String> {
    if lo > MAX_BOUND || hi.is_some_and(|h| h > MAX_BOUND) {
        return Err(format!(
            "bound {{{},{}}} exceeds this frontend's cap of {} repetitions (literal unrolling would blow up the regex size)",
            lo,
            hi.map(|h| h.to_string()).unwrap_or_default(),
            MAX_BOUND
        ));
    }
    if let Some(h) = hi {
        if h < lo {
            return Err(format!("invalid bound {{{},{}}}: upper bound is less than lower bound", lo, h));
        }
    }
    let exact = repeat_exact(&r, lo);
    match hi {
        None => Ok(Regex::seq(exact, Regex::star(r))),
        Some(h) if h == lo => Ok(exact),
        Some(h) => Ok(Regex::seq(exact, optional_extra(&r, h - lo))),
    }
}

/// `r` repeated exactly `k` times, seq'd together (`Eps` for `k == 0`).
fn repeat_exact(r: &Regex, k: u32) -> Regex {
    (0..k).fold(Regex::Eps, |acc, _| Regex::seq(acc, r.clone()))
}

/// Zero to `k` further optional copies of `r` -- the `{m,n}` upper part, `k = n - m`
fn optional_extra(r: &Regex, k: u32) -> Regex {
    if k == 0 {
        Regex::Eps
    } else {
        Regex::alt(Regex::Eps, Regex::seq(r.clone(), optional_extra(r, k - 1)))
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::{parse_deriv_std_rec, parse_pderiv_bc};
    use crate::types::flatten;

    fn t(ep: ExtPat) -> Regex {
        translate(&ep).expect("translate should succeed")
    }

    #[test]
    fn empty_is_eps() {
        assert_eq!(t(ExtPat::Empty), Regex::Eps);
    }

    #[test]
    fn char_is_lit() {
        assert_eq!(t(ExtPat::Char('a')), Regex::lit('a'));
    }

    #[test]
    fn concat_folds_left() {
        let ep = ExtPat::Concat(vec![ExtPat::Char('a'), ExtPat::Char('b'), ExtPat::Char('c')]);
        assert_eq!(t(ep), Regex::seq(Regex::seq(Regex::lit('a'), Regex::lit('b')), Regex::lit('c')));
    }

    #[test]
    fn or_folds_left() {
        let ep = ExtPat::Or(vec![ExtPat::Char('a'), ExtPat::Char('b')]);
        assert_eq!(t(ep), Regex::alt(Regex::lit('a'), Regex::lit('b')));
    }

    #[test]
    fn eps_token_is_eps() {
        assert_eq!(t(ExtPat::Eps), Regex::Eps);
    }

    #[test]
    fn never_token_is_phi() {
        assert_eq!(t(ExtPat::Never), Regex::Phi);
    }

    #[test]
    fn opt_desugars_to_alt_eps() {
        assert_eq!(t(ExtPat::Opt(Box::new(ExtPat::Char('a')), true)), Regex::alt(Regex::lit('a'), Regex::Eps));
    }

    #[test]
    fn plus_desugars_to_seq_star() {
        assert_eq!(
            t(ExtPat::Plus(Box::new(ExtPat::Char('a')), true)),
            Regex::seq(Regex::lit('a'), Regex::star(Regex::lit('a')))
        );
    }

    #[test]
    fn star_desugars_directly() {
        assert_eq!(t(ExtPat::Star(Box::new(ExtPat::Char('a')), true)), Regex::star(Regex::lit('a')));
    }

    #[test]
    fn anchors_are_eps() {
        assert_eq!(t(ExtPat::Carat), Regex::Eps);
        assert_eq!(t(ExtPat::Dollar), Regex::Eps);
    }

    #[test]
    fn any_class_matches_each_member_and_nothing_else() {
        let r = t(ExtPat::Any(vec!['a', 'b', 'c']));
        for w in ["a", "b", "c"] {
            assert!(parse_deriv_std_rec(w, &r).is_some(), "should match {:?}", w);
        }
        assert!(parse_deriv_std_rec("d", &r).is_none());
    }

    #[test]
    fn noneof_class_excludes_given_chars() {
        let r = t(ExtPat::NoneOf(vec!['a', 'b']));
        assert!(parse_deriv_std_rec("c", &r).is_some());
        assert!(parse_deriv_std_rec("a", &r).is_none());
        assert!(parse_deriv_std_rec("b", &r).is_none());
    }

    #[test]
    fn escape_d_matches_only_digits() {
        let r = t(ExtPat::Escape('d'));
        assert!(parse_deriv_std_rec("7", &r).is_some());
        assert!(parse_deriv_std_rec("x", &r).is_none());
    }

    #[test]
    fn escape_capital_d_matches_only_non_digits() {
        let r = t(ExtPat::Escape('D'));
        assert!(parse_deriv_std_rec("x", &r).is_some());
        assert!(parse_deriv_std_rec("7", &r).is_none());
    }

    #[test]
    fn escape_dot_is_literal() {
        // \. -- a punctuation escape, not a class shorthand: literal '.'
        let r = t(ExtPat::Escape('.'));
        assert_eq!(r, Regex::lit('.'));
    }

    #[test]
    fn bound_exact_three() {
        let r = t(ExtPat::Bound(Box::new(ExtPat::Char('a')), 3, Some(3), true));
        assert!(parse_deriv_std_rec("aaa", &r).is_some());
        assert!(parse_deriv_std_rec("aa", &r).is_none());
        assert!(parse_deriv_std_rec("aaaa", &r).is_none());
    }

    #[test]
    fn bound_range_two_to_four() {
        let r = t(ExtPat::Bound(Box::new(ExtPat::Char('a')), 2, Some(4), true));
        assert!(parse_deriv_std_rec("a", &r).is_none());
        assert!(parse_deriv_std_rec("aa", &r).is_some());
        assert!(parse_deriv_std_rec("aaa", &r).is_some());
        assert!(parse_deriv_std_rec("aaaa", &r).is_some());
        assert!(parse_deriv_std_rec("aaaaa", &r).is_none());
    }

    #[test]
    fn bound_unbounded_lower() {
        let r = t(ExtPat::Bound(Box::new(ExtPat::Char('a')), 2, None, true));
        assert!(parse_deriv_std_rec("a", &r).is_none());
        assert!(parse_deriv_std_rec("aa", &r).is_some());
        assert!(parse_deriv_std_rec("aaaaaaaa", &r).is_some());
    }

    #[test]
    fn bound_over_cap_is_rejected() {
        let ep = ExtPat::Bound(Box::new(ExtPat::Char('a')), 0, Some(MAX_BOUND + 1), true);
        assert!(translate(&ep).is_err());
    }

    // Regex has no lazy-quantifier concept, so translate() drops the greedy flag entirely.
    #[test]
    fn lazy_and_greedy_quantifiers_translate_identically() {
        assert_eq!(
            t(ExtPat::Star(Box::new(ExtPat::Char('a')), true)),
            t(ExtPat::Star(Box::new(ExtPat::Char('a')), false)),
        );
        assert_eq!(
            t(ExtPat::Opt(Box::new(ExtPat::Char('a')), true)),
            t(ExtPat::Opt(Box::new(ExtPat::Char('a')), false)),
        );
    }

    #[test]
    fn dot_matches_any_alphabet_char() {
        let r = t(ExtPat::Dot);
        assert!(parse_deriv_std_rec("q", &r).is_some());
        assert!(parse_deriv_std_rec("!", &r).is_some());
        assert!(parse_deriv_std_rec(" ", &r).is_some());
    }

    // \b/\B parse into ExtPat (see parse.rs's tests) but a plain Regex can't
    // represent their position-dependence, so translate() rejects them.

    #[test]
    fn word_boundary_is_rejected() {
        assert!(translate(&ExtPat::WordBoundary(true)).is_err());
        assert!(translate(&ExtPat::WordBoundary(false)).is_err());
    }

    // Round trip: a translated ExtPat's parse tree flattens back to the original input
    #[test]
    fn round_trip_through_desugared_bound_and_class() {
        let ep = ExtPat::Concat(vec![
            ExtPat::Bound(Box::new(ExtPat::Escape('d')), 1, Some(3), true),
            ExtPat::Char('.'),
        ]);
        let r = t(ep);
        for w in ["1.", "12.", "123."] {
            let tree = parse_deriv_std_rec(w, &r).unwrap_or_else(|| panic!("should match {:?}", w));
            assert_eq!(flatten(&tree), w);
            // Greedy pderiv_bc must at least agree on membership.
            assert!(parse_pderiv_bc(w, &r).is_some());
        }
    }
}
