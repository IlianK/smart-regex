//! src/parsers/deriv_std/parse.rs
//!
//! Plain-Regex POSIX parsers (deriv_std): recursive and loop variants; traced counterparts in traced.rs.

use std::time::Instant;

use crate::types::{Regex, ParseTree};
use crate::regex::deriv::deriv;
use crate::regex::nullable::nullable;
use crate::regex::mk_eps::mk_eps;
use crate::parsers::deriv_std::inject::inject;
use crate::parsers::PhaseTiming;


// RECURSIVE PARSER

/// Indexes into `chars` instead of collecting the remaining input into a
/// fresh `String` on every call: collecting a fresh `String` each time would
/// allocate and copy an O(n), O(n-1), O(n-2)... suffix at each step, O(n^2)
/// total.
fn parse_recursive_helper(r: &Regex, chars: &[char], pos: usize) -> Option<ParseTree> {
    match chars.get(pos) {
        None => {
            if nullable(r) { Some(mk_eps(r)) } else { None }
        }
        Some(&l) => {
            let r_deriv = deriv(r, l);
            let subtree = parse_recursive_helper(&r_deriv, chars, pos + 1)?;
            Some(inject(r, l, subtree))
        }
    }
}

pub fn parse_deriv_std_rec(input: &str, r: &Regex) -> Option<ParseTree> {
    let chars: Vec<char> = input.chars().collect();
    parse_recursive_helper(r, &chars, 0).map(reverse_stars)
}

/// Same parse, plus a `PhaseTiming` breakdown -- a second, independent
/// implementation of the helper above, not a thin wrapper around it:
/// `deriv` (forward) and `inject` (backward) are interleaved in a
/// single recursive descent here, not two separate passes
/// (`deriv_std_loop` below has that structure; this does not), so
/// there is no single seam to wrap in two `Instant::now()` calls.
/// Isolating them means timing each `deriv`/`inject` call individually
/// and accumulating, which adds roughly two clock reads per input
/// character on top of the parse itself -- on the order of tens of
/// nanoseconds each, negligible against the seconds-scale costs this
/// parser is used to characterise, but a real, non-zero fraction of a
/// very short, fast parse. For that reason `parse_deriv_std_rec`
/// above does not delegate to this one the way the other parsers'
/// plain functions delegate to their own `_timed` counterpart: doing
/// so would tax every ordinary call with this function's own
/// per-character measurement overhead for no benefit.
///
/// Each level also drops its own `r_deriv` -- the derivative it built
/// just to make the recursive call below -- inside a timed window
/// charged to `reconstruct_us`, instead of letting it drop implicitly
/// when this stack frame unwinds after that window closes. `r_deriv`
/// is only borrowed by the recursive call (`&r_deriv`), never moved
/// into it, so it is still ours to drop once that call returns, on
/// *either* outcome: a match, right before `inject` runs, or a
/// non-match, right before propagating `None` onward. Both branches
/// need their own drop -- a non-match reaching this level means the
/// recursive call below just returned `None`, which never reaches the
/// `inject` line at all, so folding the drop into that line's timer
/// only would leave it untimed (and, cascading, untimed at every
/// enclosing level too) on exactly the inputs that don't match, which
/// is where this fix matters least if skipped quietly and most if
/// it's wrong: half of any real corpus. Across the whole recursion
/// this accounts for the same retained-derivative-history cost
/// `deriv_std_loop` pays in one bulk drop of `expressions`, just paid
/// here in per-level slices instead. The non-match branch costs one
/// more clock read per level than the match branch already had, but
/// only on levels actually returning `None`, so a full match still
/// pays the original two reads per character, not three.
fn parse_recursive_helper_timed(
    r: &Regex,
    chars: &[char],
    pos: usize,
    derive_us: &mut u64,
    reconstruct_us: &mut u64,
) -> Option<ParseTree> {
    match chars.get(pos) {
        None => {
            let t = Instant::now();
            let result = if nullable(r) { Some(mk_eps(r)) } else { None };
            *reconstruct_us += t.elapsed().as_micros() as u64;
            result
        }
        Some(&l) => {
            let t = Instant::now();
            let r_deriv = deriv(r, l);
            *derive_us += t.elapsed().as_micros() as u64;

            let Some(subtree) = parse_recursive_helper_timed(&r_deriv, chars, pos + 1, derive_us, reconstruct_us) else {
                let t = Instant::now();
                drop(r_deriv);
                *reconstruct_us += t.elapsed().as_micros() as u64;
                return None;
            };

            let t = Instant::now();
            drop(r_deriv);
            let result = inject(r, l, subtree);
            *reconstruct_us += t.elapsed().as_micros() as u64;
            Some(result)
        }
    }
}

/// Unlike the three non-recursive parsers' `_timed` functions, the
/// initial `let chars = input.chars().collect()` here used to happen
/// before any timer started -- invisible to every phase, though still
/// inside `bench_worker`'s own outer wall-clock measurement. It is now
/// timed into `derive_us`'s own starting value, the same phase the
/// other four parsers' equivalent char-handling already falls inside
/// (see `PhaseTiming`'s doc comment): one one-time timer pair in this
/// wrapper, not per character, so the accepted two-reads-per-character
/// trade-off below is unchanged.
pub fn parse_deriv_std_rec_timed(input: &str, r: &Regex) -> (Option<ParseTree>, PhaseTiming) {
    let t_collect = Instant::now();
    let chars: Vec<char> = input.chars().collect();
    let mut derive_us = t_collect.elapsed().as_micros() as u64;
    let mut reconstruct_us = 0u64;
    let tree = parse_recursive_helper_timed(r, &chars, 0, &mut derive_us, &mut reconstruct_us);

    let t = Instant::now();
    let tree = tree.map(reverse_stars);
    reconstruct_us += t.elapsed().as_micros() as u64;

    (tree, PhaseTiming { setup_us: 0, derive_us, reconstruct_us, teardown_us: 0 })
}

/// Reverses every `Star`'s items exactly once. `inject`'s Star case
/// (see inject.rs) `push`es a new iteration onto the existing tail
/// instead of rebuilding the whole vec by `extend`, O(1) amortized
/// instead of O(current length) -- which leaves every Star's items in
/// reverse temporal order until this single O(total tree size)
/// correction pass, applied once here after the full backward pass
/// completes.
pub(super) fn reverse_stars(v: ParseTree) -> ParseTree {
    match v {
        ParseTree::Empty | ParseTree::Char(_) => v,
        ParseTree::Pair(v1, v2) => ParseTree::Pair(Box::new(reverse_stars(*v1)), Box::new(reverse_stars(*v2))),
        ParseTree::Left(v1) => ParseTree::Left(Box::new(reverse_stars(*v1))),
        ParseTree::Right(v1) => ParseTree::Right(Box::new(reverse_stars(*v1))),
        ParseTree::Star(mut items) => {
            items.reverse();
            ParseTree::Star(items.into_iter().map(reverse_stars).collect())
        }
    }
}


// LOOP PARSER

pub fn parse_deriv_std_loop(input: &str, r: &Regex) -> Option<ParseTree> {
    parse_deriv_std_loop_timed(input, r).0
}

/// Same parse, plus a `PhaseTiming` breakdown. Unlike the recursive
/// variant above, this one already has the two passes Chapter 5
/// describes as two separate loops -- the first building `expressions`
/// (forward, derive), the second building `tree` via `inject`
/// (backward, reconstruct) -- so, same as the other timed parsers,
/// wrapping each loop in one `Instant::now()`/`.elapsed()` pair is
/// enough; no per-character accumulator needed. There is no setup
/// phase distinct from derive: unlike `deriv_bc`, this parser has no
/// separate annotated representation to build first, so `setup_us` is
/// always 0, the same reason `pderiv_bc`/`pderiv_std` report 0 there.
///
/// `expressions` holds every intermediate derivative, one full `Regex`
/// per input position -- POSIX injection has to revisit each one by
/// index, unlike the bit-coded/partial-derivative parsers, which only
/// ever carry their current state forward. Dropping the whole vector
/// used to happen implicitly when this function returned, after
/// `reconstruct_us`'s own timer had already stopped; it is now dropped
/// explicitly inside a timed window instead, on *both* outcomes: after
/// the backward pass and `reverse_stars` on a match, or immediately on
/// the `!nullable` early return for a non-match, which never reaches
/// the backward pass (or that drop) at all. A non-match never runs
/// `inject`, so there is no reconstruct work to fold this into here;
/// it is added onto `derive_us` instead, since that is the phase whose
/// own work is what left `expressions` needing to be dropped.
pub fn parse_deriv_std_loop_timed(input: &str, r: &Regex) -> (Option<ParseTree>, PhaseTiming) {
    let t0 = Instant::now();
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();

    let mut expressions = Vec::with_capacity(n + 1);
    expressions.push(r.clone());

    for &c in chars.iter() {
        let current = expressions.last().unwrap();
        let next = deriv(current, c);
        expressions.push(next);
    }
    let derive_us = t0.elapsed().as_micros() as u64;

    let final_r = expressions.last().unwrap();
    if !nullable(final_r) {
        let t_drop = Instant::now();
        drop(expressions);
        let derive_us = derive_us + t_drop.elapsed().as_micros() as u64;
        return (None, PhaseTiming { setup_us: 0, derive_us, reconstruct_us: 0, teardown_us: 0 });
    }

    let t1 = Instant::now();
    let mut tree = mk_eps(expressions.last().unwrap());
    for i in (0..n).rev() {
        tree = inject(&expressions[i], chars[i], tree);
    }
    let tree = reverse_stars(tree);
    drop(expressions);
    let reconstruct_us = t1.elapsed().as_micros() as u64;

    (Some(tree), PhaseTiming { setup_us: 0, derive_us, reconstruct_us, teardown_us: 0 })
}


// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::flatten;
    use crate::parsers::deriv_bc::parse_deriv_bc;

    #[test]
    fn star_iterations_are_in_left_to_right_order() {
        // (a|b)* on "abab": pins down iteration order -- a reversed-order
        // implementation would give [b,a,b,a] instead of the correct
        // [a,b,a,b].
        let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));

        let tree_rec = parse_deriv_std_rec("abab", &r).unwrap();
        let tree_loop = parse_deriv_std_loop("abab", &r).unwrap();
        assert_eq!(tree_rec, tree_loop);
        assert_eq!(flatten(&tree_rec), "abab");
        assert_eq!(
            tree_rec,
            ParseTree::Star(vec![
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Char('b'))),
                ParseTree::Left(Box::new(ParseTree::Char('a'))),
                ParseTree::Right(Box::new(ParseTree::Char('b'))),
            ])
        );
        assert_eq!(tree_rec, parse_deriv_bc("abab", &r).unwrap());
    }

    #[test]
    fn nested_star_iterations_match_bc_order() {
        // ((a|b)(a|b))* on "abba": two-character iterations, so a
        // within-iteration order bug (not just a between-iteration one)
        // would also show up here.
        let pair = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
        );
        let r = Regex::star(pair);

        let tree_rec = parse_deriv_std_rec("abba", &r).unwrap();
        let tree_loop = parse_deriv_std_loop("abba", &r).unwrap();
        assert_eq!(tree_rec, tree_loop);
        assert_eq!(flatten(&tree_rec), "abba");
        assert_eq!(tree_rec, parse_deriv_bc("abba", &r).unwrap());
    }

    #[test]
    fn rec_timed_matches_untimed_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_deriv_std_rec("aaa", &r);
        let (timed, timing) = parse_deriv_std_rec_timed("aaa", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.setup_us, 0, "deriv_std has no setup phase");
        // teardown_us is always 0 as directly returned from any of the
        // five parser functions -- only bench_worker fills it in, once
        // it knows what its own caller does with the returned tree.
        assert_eq!(timing.teardown_us, 0);
    }

    #[test]
    fn rec_timed_matches_untimed_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_deriv_std_rec("aab", &r);
        let (timed, timing) = parse_deriv_std_rec_timed("aab", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.setup_us, 0);
        assert_eq!(timing.teardown_us, 0);
    }

    // Exercises the timed recursive helper's new `drop(r_deriv)` across
    // several levels of real recursion (not just the single-character
    // cases above), on a pattern with Seq/Alt/Star all nested together
    // -- confirms dropping each level's own derivative early, right
    // before `inject` runs, changes nothing about the tree `inject`
    // still builds from `r`/`l`/`subtree`.
    #[test]
    fn rec_timed_matches_untimed_on_nested_pattern() {
        let pair = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
        );
        let r = Regex::star(pair);
        let plain = parse_deriv_std_rec("abba", &r);
        let (timed, timing) = parse_deriv_std_rec_timed("abba", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.teardown_us, 0);
    }

    #[test]
    fn loop_timed_matches_untimed_on_success() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_deriv_std_loop("aaa", &r);
        let (timed, timing) = parse_deriv_std_loop_timed("aaa", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.setup_us, 0, "deriv_std has no setup phase");
        assert_eq!(timing.teardown_us, 0);
    }

    #[test]
    fn loop_timed_matches_untimed_on_failure() {
        let r = Regex::star(Regex::lit('a'));
        let plain = parse_deriv_std_loop("aab", &r);
        let (timed, timing) = parse_deriv_std_loop_timed("aab", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.setup_us, 0);
        assert_eq!(timing.teardown_us, 0);
    }

    // Exercises the timed loop variant's new `drop(expressions)` on a
    // pattern whose `expressions` history holds more than one shape of
    // derivative across its run -- confirms dropping the whole
    // retained-derivative vector right after the backward pass and
    // `reverse_stars` are done with it changes nothing about the tree
    // already built from it.
    #[test]
    fn loop_timed_matches_untimed_on_nested_pattern() {
        let pair = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
        );
        let r = Regex::star(pair);
        let plain = parse_deriv_std_loop("abba", &r);
        let (timed, timing) = parse_deriv_std_loop_timed("abba", &r);
        assert_eq!(plain, timed);
        assert_eq!(timing.teardown_us, 0);
    }

    #[test]
    fn loop_timed_reports_zero_reconstruct_on_failure() {
        // The loop variant returns before the backward pass ever runs
        // on a non-match, unlike the recursive variant, whose base
        // case always times its own nullable check regardless of the
        // outcome -- so only the loop variant's reconstruct_us is
        // exactly, not just approximately, zero here.
        let (tree, timing) = parse_deriv_std_loop_timed("aab", &Regex::star(Regex::lit('a')));
        assert_eq!(tree, None);
        assert_eq!(timing.reconstruct_us, 0);
        assert_eq!(timing.teardown_us, 0);
    }

    #[test]
    fn rec_and_loop_timed_agree_with_each_other() {
        let pair = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
            Regex::alt(Regex::lit('a'), Regex::lit('b')),
        );
        let r = Regex::star(pair);
        let (tree_rec, _) = parse_deriv_std_rec_timed("abba", &r);
        let (tree_loop, _) = parse_deriv_std_loop_timed("abba", &r);
        assert_eq!(tree_rec, tree_loop);
    }
}
