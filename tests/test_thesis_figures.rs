//! regex-engine/tests/test_thesis_figures.rs

use regex_engine::frontend::{parse_dataset_pattern, parse_pattern};
use regex_engine::parsers::deriv_bc::{
    deriv::deriv_bc, internalize::internalize, simplify::simp,
};
use regex_engine::parsers::pderiv_bc::pderiv::pderiv_bc;
use regex_engine::regex::deriv::deriv;
use regex_engine::types::{ARegex, Regex};


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

/// `(a + (b + ab))*`, right-associated, as the test suite builds `paper_r2`.
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
        vec![8, 12, 17, 25, 35, 51, 71, 103, 143, 207, 287, 415, 575, 831, 1151, 1663, 2303],
        "Figure 5.3, series 1: simp does not bound (a+(b+ab))*"
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

/// Section 5.5 prose: every two characters take the size s to 2s+1, so the
/// growth is Theta(2^(n/2)), reaching 2303 at 16 characters and 36863 at 24.
#[test]
fn section_5_5_growth_is_exponential() {
    let s = bitcoded_simplified(&paper_r2(), &"ab".repeat(12));
    assert_eq!(s[16], 2303, "Section 5.5 quotes 2303 at step 16");
    assert_eq!(s[24], 36863, "Section 5.5 quotes 36863 at step 24");
    for k in (4..=24).step_by(2) {
        assert_eq!(
            s[k],
            2 * s[k - 2] + 1,
            "step {} should be 2*step {} + 1",
            k,
            k - 2
        );
    }
}

/// Section 5.5 prose: a single `simp` pass already is the fixpoint, so the
/// growth is not an artefact of stopping too early.
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

/// Section 5.5 table: every alternation branch is identical modulo its bits,
/// which is why `r + r = r` never fires.
#[test]
fn section_5_5_branches_are_identical_modulo_bits() {
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
            ARegex::Eps(_) => ARegex::Eps(vec![]),
            ARegex::Lit(_, c) => ARegex::Lit(vec![], *c),
            ARegex::Alt(_, a, b) => ARegex::Alt(vec![], Box::new(strip(a)), Box::new(strip(b))),
            ARegex::Seq(_, a, b) => ARegex::Seq(vec![], Box::new(strip(a)), Box::new(strip(b))),
            ARegex::Star(_, a) => ARegex::Star(vec![], Box::new(strip(a))),
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
        vec![2, 4, 8, 16, 32, 64],
        "Section 5.5 table, row 'Branches'"
    );
    assert_eq!(
        picked.iter().map(|t| t.1).collect::<Vec<_>>(),
        vec![2, 4, 8, 16, 32, 64],
        "Section 5.5 table, row 'Distinct branches'"
    );
    assert_eq!(
        picked.iter().map(|t| t.2).collect::<Vec<_>>(),
        vec![1, 1, 1, 1, 1, 1],
        "Section 5.5 table, row 'Distinct ignoring bits'"
    );
}

/// Section 5.5 prose: the figures are the same for the other grouping of the
/// three-way alternation, which is what the CLI spelling "(a|b|ab)*" parses to.
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

#[test]
fn fig_6_3_frontier_doubles_while_distinct_stays_bounded() {
    let rows = frontier_sizes(&paper_r2(), &"ab".repeat(7));
    assert_eq!(
        rows.iter().map(|t| t.0).collect::<Vec<_>>(),
        vec![1, 2, 2, 4, 4, 8, 8, 16, 16, 32, 32, 64, 64, 128, 128],
        "Figure 6.3, series 1: strands in the frontier"
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

#[test]
fn fig_7_4_search_padding_sizes() {
    let cases: &[(&str, usize)] = &[
        ("^abc$", 9),
        ("^abc", 204),
        ("abc$", 204),
        ("abc", 399),
        (r"[a-c]+\d{2}", 448),
    ];
    for &(pat, want) in cases {
        let r = parse_dataset_pattern(pat).unwrap_or_else(|e| panic!("{:?}: {}", pat, e));
        assert_eq!(
            size_regex(&r),
            want,
            "Figure 7.3/7.4: lowered size of {:?} under substring search",
            pat
        );
    }
}

/// Section 7.4 caption: one `wildcard_run()` is 196 nodes.
#[test]
fn section_7_4_wildcard_run_is_196_nodes() {
    let padded = size_regex(&parse_dataset_pattern("^abc").unwrap());
    let core = size_regex(&parse_pattern("^abc").unwrap());
    assert_eq!(padded - core - 1, 196);
}

/// Section 7.5: case folding adds two nodes per folded letter.
#[test]
fn section_7_5_case_folding_cost() {
    use regex_engine::frontend::parse_pcre_rule;
    let plain = size_regex(&parse_pcre_rule("/abc/").unwrap());
    let folded = size_regex(&parse_pcre_rule("/abc/i").unwrap());
    assert_eq!(plain, 399);
    assert_eq!(folded, 405);
}
