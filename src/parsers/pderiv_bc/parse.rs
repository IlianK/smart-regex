//! src/parsers/pderiv_bc/parse.rs
//! 
//! Bit-coded partial-derivative parser (pDerivBC / parsePDerivBC), GREEDY; traced variant in traced.rs.

use std::time::Instant;
use crate::types::{Regex, ParseTree, AppendBits};
use crate::regex::nullable::nullable;
use crate::parsers::pderiv_bc::pderiv::{pderiv_bc, pderiv_bc_timed, mk_eps_bits, nub2};
use crate::parsers::PhaseTiming;
use crate::regex::decode::decode;

/// One step of `parsePDerivBC2`: replace the frontier with union of
/// `pderiv_bc(x, ·)`, then dedup across the whole combined frontier
/// (not just within one parent's own expansion). The reference slide's
/// `parsePDerivBC2` omits this, but its own `runBC_NFA`/`go` applies
/// `nub2` at exactly this point, matching Sulzmann & Lu 2012's Def. 8
/// ("duplicate states are removed, only the left-most state is kept")
/// and this project's own `demo_growth_pderiv` finding: without it, the
/// frontier doubles every character on `(a|b|ab)*` even though the
/// number of distinct residuals stays bounded.
///
/// Each strand's bits accumulate across the whole input so far; a plain
/// `Vec<bool>` clone-then-extend here is O(current length) per step,
/// O(n^2) over n characters -- `AppendBits::append_bits` is O(1)
/// regardless (see src/types/bits.rs), grafting the two existing chains
/// together instead of flattening either one. `pderiv_bc`'s own `extra`
/// is itself an unmaterialized `AppendBits` (its per-leaf prefix out of
/// a possibly-wide alternation, see `pderiv_bc::flatten_alt`), so the
/// graft here keeps it that way rather than forcing a flatten on every
/// character step.
pub(super) fn step_frontier(frontier: &[(Regex, AppendBits)], x: char) -> Vec<(Regex, AppendBits)> {
    let mut out = Vec::new();
    for (r, bits) in frontier {
        for (r_next, extra) in pderiv_bc(r, x) {
            out.push((r_next, bits.append_bits(&extra)));
        }
    }
    nub2(out)
}

/// Same step, plus threading `pderiv_bc_timed`'s `tag_us` accumulator
/// through -- see that function's own doc comment for what it counts.
pub(super) fn step_frontier_timed(
    frontier: &[(Regex, AppendBits)],
    x: char,
    tag_us: &mut u64,
) -> Vec<(Regex, AppendBits)> {
    let mut out = Vec::new();
    for (r, bits) in frontier {
        for (r_next, extra) in pderiv_bc_timed(r, x, tag_us) {
            out.push((r_next, bits.append_bits(&extra)));
        }
    }
    nub2(out)
}

/// `filter (\(r,_) -> nullable r) rs`, take head, and append `mkEpsBC`.
pub(super) fn select_bits(frontier: &[(Regex, AppendBits)]) -> Option<Vec<bool>> {
    for (r, bits) in frontier {
        if nullable(r) {
            return Some(bits.append(&mk_eps_bits(r)).to_vec());
        }
    }
    None
}

pub fn parse_pderiv_bc(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_pderiv_bc_timed(input, r).0
}

/// Same parse, plus `setup_us`/`derive_us`/`reconstruct_us` sized to be
/// genuinely comparable to `deriv_bc`'s encode/derive/decode, not just
/// positionally aligned with them. `setup_us` here is the trivial
/// initial clone (`r.clone()` into the starting frontier) PLUS the
/// total time spent in `flatten_alt` tagging `Alt` branches with bit
/// prefixes across the whole parse: the same operation `internalize`
/// performs once, upfront, for `deriv_bc`, just repeated fresh on
/// every character here instead. `derive_us` is the rest of the
/// per-character work (the actual `pderiv_bc` transition computation
/// plus `nub2` dedup) with that tagging time subtracted back out, so
/// the three fields still sum to the true total and nothing is
/// double-counted. `reconstruct_us` is unchanged, a genuine decode:
/// `decode(r, &bits)` interprets the accumulated bit string against
/// the pattern. See `pderiv_bc_timed`'s own doc comment for exactly
/// what does and doesn't count as tagging.
pub fn parse_pderiv_bc_timed(input: &str, r: &Regex) -> (Option<ParseTree>, PhaseTiming) {
    let t_setup = Instant::now();
    let mut frontier = vec![(r.clone(), AppendBits::empty())];
    let clone_us = t_setup.elapsed().as_micros() as u64;

    let mut tag_us = 0u64;
    let t0 = Instant::now();
    for c in input.chars() {
        frontier = step_frontier_timed(&frontier, c, &mut tag_us);
        if frontier.is_empty() {
            let derive_us = (t0.elapsed().as_micros() as u64).saturating_sub(tag_us);
            let setup_us = clone_us + tag_us;
            return (None, PhaseTiming { setup_us, derive_us, reconstruct_us: 0 });
        }
    }
    let derive_us = (t0.elapsed().as_micros() as u64).saturating_sub(tag_us);
    let setup_us = clone_us + tag_us;

    let t1 = Instant::now();
    let Some(bits) = select_bits(&frontier) else {
        return (None, PhaseTiming { setup_us, derive_us, reconstruct_us: 0 });
    };
    let tree = decode(r, &bits);
    let reconstruct_us = t1.elapsed().as_micros() as u64;

    (Some(tree), PhaseTiming { setup_us, derive_us, reconstruct_us })
}


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Regex, flatten};
    use crate::parsers::deriv_std::parse_deriv_std_rec;

    #[test]
    fn empty_regex_empty_input() {
        assert_eq!(parse_pderiv_bc("", &Regex::Eps), Some(ParseTree::Empty));
    }

    #[test]
    fn literal_matches() {
        let r = Regex::lit('a');
        assert_eq!(parse_pderiv_bc("a", &r), Some(ParseTree::Char('a')));
    }

    #[test]
    fn no_match_is_none() {
        assert_eq!(parse_pderiv_bc("b", &Regex::lit('a')), None);
    }

    #[test]
    fn timed_matches_untimed_output() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_pderiv_bc("aaa", &r);
        let (timed, _timing) = parse_pderiv_bc_timed("aaa", &r);
        assert_eq!(plain, timed);
    }

    // Same check, but through an Alt-containing pattern -- the only
    // one that exercises step_frontier_timed's/pderiv_bc_timed's new
    // tag_us threading end to end, which the Star-only case above
    // never touches.
    #[test]
    fn timed_matches_untimed_output_with_alt() {
        let r = Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b')));
        let plain = parse_pderiv_bc("ab", &r);
        let (timed, _timing) = parse_pderiv_bc_timed("ab", &r);
        assert_eq!(plain, timed);
    }

    #[test]
    fn timed_reports_zero_decode_on_no_match() {
        let (tree, timing) = parse_pderiv_bc_timed("b", &Regex::lit('a'));
        assert_eq!(tree, None);
        assert_eq!(timing.reconstruct_us, 0);
    }

    #[test]
    fn star_three_iterations() {
        let r = Regex::star(Regex::lit('a'));
        let tree = parse_pderiv_bc("aaa", &r).unwrap();
        assert_eq!(flatten(&tree), "aaa");
        assert_eq!(tree, parse_deriv_std_rec("aaa", &r).unwrap());
    }

    #[test]
    fn paper_r1_computes_greedy_not_posix() {
        // (a + ab)(b + eps) on "ab"
        let r = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
            Regex::alt(Regex::lit('b'), Regex::Eps),
        );

        let posix = parse_deriv_std_rec("ab", &r);
        let greedy = parse_pderiv_bc("ab", &r);

        assert!(posix.is_some() && greedy.is_some(), "both should match");
        assert_eq!(flatten(posix.as_ref().unwrap()), "ab");
        assert_eq!(flatten(greedy.as_ref().unwrap()), "ab");

        // POSIX: the longer "ab" alternative wins (rule A1)
        assert_eq!(
            posix,
            Some(ParseTree::Pair(
                Box::new(ParseTree::Right(Box::new(ParseTree::Pair(
                    Box::new(ParseTree::Char('a')),
                    Box::new(ParseTree::Char('b')),
                )))),
                Box::new(ParseTree::Right(Box::new(ParseTree::Empty))),
            ))
        );
        // Greedy: the leftmost alternative 'a' wins, committed to immediately
        assert_eq!(
            greedy,
            Some(ParseTree::Pair(
                Box::new(ParseTree::Left(Box::new(ParseTree::Char('a')))),
                Box::new(ParseTree::Left(Box::new(ParseTree::Char('b')))),
            ))
        );
        assert_ne!(posix, greedy, "this is exactly the documented divergence");
    }

    // Both parsers still agree on Some/None; only which parse tree is selected differs.
    #[test]
    fn paper_r2_computes_greedy_not_posix() {
        // (a + (b + ab))* on "ab"
        let r = Regex::star(Regex::alt(
            Regex::lit('a'),
            Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        ));

        let posix = parse_deriv_std_rec("ab", &r);
        let greedy = parse_pderiv_bc("ab", &r);

        assert!(posix.is_some() && greedy.is_some(), "both should match");
        assert_eq!(flatten(posix.as_ref().unwrap()), "ab");
        assert_eq!(flatten(greedy.as_ref().unwrap()), "ab");

        // POSIX: one iteration, the longer "ab" alternative wins (rule A1)
        assert_eq!(
            posix,
            Some(ParseTree::Star(vec![ParseTree::Right(Box::new(ParseTree::Right(
                Box::new(ParseTree::Pair(
                    Box::new(ParseTree::Char('a')),
                    Box::new(ParseTree::Char('b')),
                ))
            )))]))
        );
        // Greedy: two iterations, leftmost alternative wins at each step
        assert_eq!(
            greedy,
            Some(ParseTree::Star(vec![
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Left(Box::new(ParseTree::Char('b'))))),
            ]))
        );
        assert_ne!(posix, greedy, "this is exactly the documented divergence");
    }
}
