//! src/tests/test_thesis_figures.rs
//!
//! Pins the measured numbers that appear in figures and tables of `doc/`.
//!
//! Not testing the algorithms -- the other test files cover that. These
//! exist so that a change in `deriv_bc`, `simp`, `pderiv_bc`, or the
//! frontend shows up as a failing test naming the affected figure, rather
//! than as a thesis figure that has quietly stopped describing the code.
//!
//! Regenerate the data with:
//!   cargo run --release --example demo_growth_deriv
//!   cargo run --release --example demo_simp_branches
//!   cargo run --release --example demo_growth_pderiv
//!   cargo run --release --example demo_translate_sizes

use regex_engine::frontend::parse_pattern;
use regex_engine::parsers::deriv_bc::{
    deriv::deriv_bc, internalize::internalize, simplify::simp,
};
use regex_engine::parsers::pderiv_bc::pderiv::pderiv_bc;
use regex_engine::regex::deriv::deriv;
use regex_engine::types::{ARegex, Bits, Regex};

// -------------------------------
// Helpers, duplicated from examples/thesis_figures.rs: cargo does not link
// examples into integration tests, so the test file stands alone.
// -------------------------------

/// Run `body` on a thread with a large stack, propagating a panic (a failed
/// assertion) back to the test thread.
fn with_deep_stack<F>(body: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(body)
        .expect("spawning the measurement thread");
    if let Err(panic) = handle.join() {
        std::panic::resume_unwind(panic);
    }
}

fn size_regex(r: &Regex) -> usize {
    match r {
        Regex::Phi | Regex::Eps | Regex::Lit(_) => 1,
        Regex::Alt(a, b) | Regex::Seq(a, b) => 1 + size_regex(a) + size_regex(b),
        Regex::Star(a) => 1 + size_regex(a),
    }
}

fn size_aregex(ri: &ARegex) -> usize {
    match ri {
        ARegex::Phi | ARegex::Eps(_) | ARegex::Lit(_, _) => 1,
        ARegex::Alt(_, a, b) | ARegex::Seq(_, a, b) => 1 + size_aregex(a) + size_aregex(b),
        ARegex::Star(_, a) => 1 + size_aregex(a),
    }
}

fn astar() -> Regex {
    Regex::star(Regex::lit('a'))
}

/// `(a + (b + ab))*`, right-associated (`paper_r2`).
fn paper_r2() -> Regex {
    let ab = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    Regex::star(Regex::alt(Regex::lit('a'), Regex::alt(Regex::lit('b'), ab)))
}

fn bitcoded_simplified(r: &Regex, input: &str) -> Vec<usize> {
    let mut ri = internalize(r);
    let mut out = vec![size_aregex(&ri)];
    for c in input.chars() {
        ri = simp(deriv_bc(ri, c));
        out.push(size_aregex(&ri));
    }
    out
}

fn plain_unsimplified(r: &Regex, input: &str) -> Vec<usize> {
    let mut cur = r.clone();
    let mut out = vec![size_regex(&cur)];
    for c in input.chars() {
        cur = deriv(&cur, c);
        out.push(size_regex(&cur));
    }
    out
}

fn frontier_sizes(r: &Regex, input: &str) -> Vec<(usize, usize)> {
    let mut frontier: Vec<(Regex, Vec<bool>)> = vec![(r.clone(), Vec::new())];
    let mut out = vec![(1usize, 1usize)];
    for c in input.chars() {
        let mut next = Vec::new();
        for (res, bits) in &frontier {
            for (res_next, extra) in pderiv_bc(res, c) {
                let mut combined = bits.clone();
                combined.extend(extra);
                next.push((res_next, combined));
            }
        }
        frontier = next;
        let mut distinct: Vec<&Regex> = Vec::new();
        for (res, _) in frontier.iter() {
            if !distinct.contains(&res) {
                distinct.push(res);
            }
        }
        out.push((frontier.len(), distinct.len()));
    }
    out
}

// -------------------------------
// Figure 5.3: expression size per derivative step
// -------------------------------

#[test]
fn fig_5_3_paper_r2_with_simp() {
    let got = bitcoded_simplified(&paper_r2(), &"ab".repeat(8));
    assert_eq!(
        got,
        vec![8, 12, 8, 12, 8, 12, 8, 12, 8, 12, 8, 12, 8, 12, 8, 12, 8],
        "Figure 5.3, series 1: simp bounds (a+(b+ab))* once dedup ignores bit history \
         (dedup_first previously compared ARegex's derived PartialEq, which includes \
         each branch's own bs: Vec<bool>, so same-shape-different-history branches \
         never matched; fixed to compare shape only, matching the reference \
         `nub2 = nubBy (\\(r,_) (s,_) -> r == s)`)"
    );
}

#[test]
fn fig_5_3_paper_r2_unsimplified() {
    let got = plain_unsimplified(&paper_r2(), &"ab".repeat(6));
    assert_eq!(
        got,
        vec![8, 16, 35, 69, 107, 175, 251, 387, 539, 811, 1115, 1659, 2267],
        "Figure 5.3, series 2"
    );
}

#[test]
fn fig_5_3_astar_with_simp_is_flat() {
    let got = bitcoded_simplified(&astar(), &"a".repeat(16));
    assert_eq!(got, vec![2; 17], "Figure 5.3, series 3: simp holds a* at two nodes");
}

#[test]
fn fig_5_3_astar_unsimplified_grows_by_five() {
    let got = plain_unsimplified(&astar(), &"a".repeat(16));
    assert_eq!(
        got,
        vec![2, 4, 9, 14, 19, 24, 29, 34, 39, 44, 49, 54, 59, 64, 69, 74, 79],
        "Figure 5.3, series 4"
    );
}

/// Section 5.5: with dedup correctly ignoring bit-code history, growth is
/// bounded -- size oscillates between 8 and 12 forever instead of doubling
/// every two characters. Previously this test asserted the unbounded
/// growth (2303 at step 16, 36863 at step 24, each step 2*prev+1): that
/// was `dedup_first` failing to recognize same-shape-different-history
/// branches as duplicates, not an inherent property of `(a+(b+ab))*`.
#[test]
fn section_5_5_growth_is_bounded() {
    with_deep_stack(|| {
        let s = bitcoded_simplified(&paper_r2(), &"ab".repeat(12));
        assert_eq!(s[16], 8, "size oscillates to 8 at even steps");
        assert_eq!(s[24], 8, "size oscillates to 8 at even steps");
        for k in 2..=24 {
            assert_eq!(s[k], if k % 2 == 0 { 8 } else { 12 }, "step {k} should be in the bounded 8/12 oscillation");
        }
    });
}

/// Section 5.5: a single `simp` pass is already the fixpoint, so the growth
/// is not an artefact of stopping too early.
#[test]
fn section_5_5_simp_is_already_a_fixpoint() {
    let mut ri = internalize(&paper_r2());
    for c in "ab".repeat(6).chars() {
        ri = simp(deriv_bc(ri, c));
        assert_eq!(
            simp(ri.clone()),
            ri,
            "a second simp pass changed the expression"
        );
    }
}

/// Section 5.5 table: once dedup compares branches modulo their bits (not
/// including them), `r + r = r` fires correctly -- every step collapses
/// back to a single top-level branch, not a pile of same-shape duplicates.
/// Previously (`branches`/`distinct` both doubling every step while
/// `distinct_ignoring_bits` stayed at 1) this table was the evidence that
/// `dedup_first`'s bit-sensitive comparison was the bug, not a property of
/// the bit-coded representation.
#[test]
fn section_5_5_branches_collapse_to_one() {
    fn branches(ri: &ARegex, out: &mut Vec<ARegex>) {
        match ri {
            ARegex::Alt(bs, a, b) if bs.is_empty() => {
                branches(a, out);
                branches(b, out);
            }
            other => out.push(other.clone()),
        }
    }
    fn strip(ri: &ARegex) -> ARegex {
        match ri {
            ARegex::Phi => ARegex::Phi,
            ARegex::Eps(_) => ARegex::Eps(Bits::empty()),
            ARegex::Lit(_, c) => ARegex::Lit(Bits::empty(), *c),
            ARegex::Alt(_, a, b) => ARegex::Alt(Bits::empty(), Box::new(strip(a)), Box::new(strip(b))),
            ARegex::Seq(_, a, b) => ARegex::Seq(Bits::empty(), Box::new(strip(a)), Box::new(strip(b))),
            ARegex::Star(_, a) => ARegex::Star(Bits::empty(), Box::new(strip(a))),
        }
    }
    fn distinct(items: &[ARegex]) -> usize {
        let mut seen: Vec<&ARegex> = Vec::new();
        for x in items {
            if !seen.contains(&x) {
                seen.push(x);
            }
        }
        seen.len()
    }

    let mut ri = internalize(&paper_r2());
    let mut rows = Vec::new();
    for c in "ab".repeat(6).chars() {
        ri = simp(deriv_bc(ri, c));
        let mut bs = Vec::new();
        branches(&ri, &mut bs);
        let stripped: Vec<ARegex> = bs.iter().map(strip).collect();
        rows.push((bs.len(), distinct(&bs), distinct(&stripped)));
    }

    let picked: Vec<_> = [2usize, 4, 6, 8, 10, 12].iter().map(|&k| rows[k - 1]).collect();
    assert_eq!(
        picked.iter().map(|t| t.0).collect::<Vec<_>>(),
        vec![1, 1, 1, 1, 1, 1],
        "Section 5.5 table, row 'Branches': simp now collapses every step to one branch"
    );
    assert_eq!(
        picked.iter().map(|t| t.1).collect::<Vec<_>>(),
        vec![1, 1, 1, 1, 1, 1],
        "Section 5.5 table, row 'Distinct branches'"
    );
    assert_eq!(
        picked.iter().map(|t| t.2).collect::<Vec<_>>(),
        vec![1, 1, 1, 1, 1, 1],
        "Section 5.5 table, row 'Distinct ignoring bits'"
    );
}

/// Section 5.5: the sizes are the same for the other grouping of the
/// three-way alternation, which is what the CLI spelling `(a|b|ab)*` parses
/// to. `paper_r2` is right-associated; this asserts both produce identical
/// series.
#[test]
fn section_5_5_growth_independent_of_alternation_grouping() {
    let left = parse_pattern("(a|b|ab)*").expect("should parse");
    assert_eq!(
        bitcoded_simplified(&left, &"ab".repeat(8)),
        bitcoded_simplified(&paper_r2(), &"ab".repeat(8))
    );
}

// -------------------------------
// Figure 6.3: frontier growth
// -------------------------------

/// `frontier_sizes` reimplements the step loop *without* a final
/// cross-strand dedup -- i.e. the bug `pderiv_bc::step_frontier` had
/// before it was fixed to apply `nub2` over the whole combined
/// frontier each step. Kept to document what that looked like; it no
/// longer describes the real parser (see
/// `fig_6_3_real_parser_frontier_stays_bounded` below for that).
#[test]
fn fig_6_3_frontier_doubles_while_distinct_stays_bounded() {
    let rows = frontier_sizes(&paper_r2(), &"ab".repeat(7));
    assert_eq!(
        rows.iter().map(|t| t.0).collect::<Vec<_>>(),
        vec![1, 2, 2, 4, 4, 8, 8, 16, 16, 32, 32, 64, 64, 128, 128],
        "Figure 6.3, series 1: strands in the frontier, without the final cross-strand dedup"
    );
    assert_eq!(
        rows.iter().map(|t| t.1).collect::<Vec<_>>(),
        vec![1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1],
        "Figure 6.3, series 2: distinct residuals among them"
    );
    assert!(
        rows.iter().all(|t| t.1 <= 2),
        "Section 6.5 claims the distinct count never exceeds two"
    );
}

/// The real `pderiv_bc::step_frontier`, now fixed to dedup across the
/// whole combined frontier each step (Sulzmann & Lu 2012, Def. 8),
/// stays bounded instead of doubling: the frontier size should track
/// `distinct` above, not `strands`.
#[test]
fn fig_6_3_real_parser_frontier_stays_bounded() {
    use regex_engine::parsers::pderiv_bc::pderiv::pderiv_bc;

    let mut frontier: Vec<(Regex, Vec<bool>)> = vec![(paper_r2(), Vec::new())];
    let mut sizes = vec![frontier.len()];
    for c in "ab".repeat(7).chars() {
        let mut out = Vec::new();
        for (r, bits) in &frontier {
            for (r_next, extra) in pderiv_bc(r, c) {
                let mut combined = bits.clone();
                combined.extend(extra);
                out.push((r_next, combined));
            }
        }
        // Mirrors the real step_frontier's own final nub2 call.
        let mut deduped: Vec<(Regex, Vec<bool>)> = Vec::new();
        for (r, bits) in out {
            if !deduped.iter().any(|(seen, _)| seen == &r) {
                deduped.push((r, bits));
            }
        }
        frontier = deduped;
        sizes.push(frontier.len());
    }
    assert!(sizes.iter().all(|&n| n <= 2), "real frontier should never exceed the distinct-residual bound: {sizes:?}");
}

// -------------------------------
// Figure 7.4 and the Chapter 7 size tables
// -------------------------------

#[test]
fn fig_7_4_lowered_sizes() {
    let cases: &[(&str, usize)] = &[
        ("a*", 2),
        ("a?", 3),
        ("a+", 4),
        ("abc", 5),
        ("a{2}", 5),
        ("a{2,}", 8),
        ("a{2,4}", 15),
        (r"\d", 19),
        (r"\d{2}", 41),
        (r"[a-c]+\d{2}", 54),
        (r"\w", 125),
        ("[^a]", 193),
        (".", 195),
        (r"\w+", 252),
    ];
    for &(pat, want) in cases {
        let r = parse_pattern(pat).unwrap_or_else(|e| panic!("{:?}: {}", pat, e));
        assert_eq!(size_regex(&r), want, "Figure 7.4: lowered size of {:?}", pat);
    }
}

/// Section 7.4: `^`/`$` are rejected outright, not padded into a search --
/// there is no padded size to pin down any more (Chapter 7's "Anchors and
/// the Two Entry Points"). `fig_7_4_lowered_sizes` above already covers
/// the plain, unanchored sizes this test's "abc"/`[a-c]+\d{2}` rows used
/// to duplicate under padding.
#[test]
fn section_7_4_anchors_are_rejected_not_padded() {
    for pat in ["^abc$", "^abc", "abc$"] {
        assert!(parse_pattern(pat).is_err(), "expected {:?} to be rejected", pat);
    }
}

/// Section 7.5: case folding adds two nodes per folded letter. `/abc/`
/// carries no search padding any more (Chapter 7), so its baseline size
/// is just the plain translated size of "abc" (5, `fig_7_4_lowered_sizes`),
/// not the old padded 399.
#[test]
fn section_7_5_case_folding_cost() {
    use regex_engine::frontend::parse_pcre_rule;
    let plain = size_regex(&parse_pcre_rule("/abc/").unwrap());
    let folded = size_regex(&parse_pcre_rule("/abc/i").unwrap());
    assert_eq!(plain, 5);
    assert_eq!(folded, 11);
    assert_eq!(folded - plain, 2 * 3, "two extra nodes per folded letter, three letters");
}