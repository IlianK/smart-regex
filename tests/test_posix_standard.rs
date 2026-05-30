// tests/test_posix_standard.rs
//
// Integration tests for src/posix/standard/
//   - parse_recursive  (posix/standard/parse.rs)
//   - parse_loop       (posix/standard/parse.rs)
//
// All paper examples are taken from Sulzmann & Lu (flops14-extended) 
//
// Run:  cargo test --test test_posix_standard

mod common;
use common::{assert_round_trip, assert_parsers_agree, paper_r1, paper_r2};

use regex_engine::{Regex, ParseTree};
use regex_engine::posix::{parse_recursive, parse_loop};

// ============================================================================
// parse_recursive — correctness
// ============================================================================

#[test]
fn parse_recursive_empty_word_on_eps() {
    let r = Regex::Eps;
    let tree = parse_recursive("", &r).expect("should match");
    assert_eq!(tree, ParseTree::Empty);
}

#[test]
fn parse_recursive_single_literal() {
    let r = Regex::lit('a');
    let tree = parse_recursive("a", &r).expect("should match");
    assert_eq!(tree, ParseTree::Char('a'));
    assert_round_trip(&tree, "a");
}

#[test]
fn parse_recursive_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    let tree = parse_recursive("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");
    assert!(matches!(tree, ParseTree::Pair(_, _)));
}

#[test]
fn parse_recursive_star_empty() {
    let r = Regex::star(Regex::lit('a'));
    let tree = parse_recursive("", &r).expect("should match");
    assert_eq!(tree, ParseTree::Star(vec![]));
}

#[test]
fn parse_recursive_star_three_chars() {
    let r = Regex::star(Regex::lit('a'));
    let tree = parse_recursive("aaa", &r).expect("should match");
    assert_round_trip(&tree, "aaa");
    assert!(matches!(&tree, ParseTree::Star(v) if v.len() == 3));
}

#[test]
fn parse_recursive_returns_none_on_no_match() {
    let r = Regex::lit('a');
    assert!(parse_recursive("b", &r).is_none());
    assert!(parse_recursive("", &r).is_none());
}

#[test]
fn parse_recursive_phi_never_matches() {
    assert!(parse_recursive("",  &Regex::Phi).is_none());
    assert!(parse_recursive("a", &Regex::Phi).is_none());
}

// ── Paper example: (a + ab)(b + ε) on "ab" ─────────────────────────────────

#[test]
fn parse_recursive_paper_r1_on_ab() {
    // Paper result (flops14 p.9):
    //   (Right(a,b), Right())
    //   i.e. Pair(Right(Pair(Char('a'),Char('b'))), Right(Empty))
    //
    // Outer Seq → Pair
    // Left factor (a + ab) takes right branch "ab" → Right(Pair(a,b))
    // Right factor (b + ε) takes right branch ε   → Right(Empty)
    let r = paper_r1();
    let tree = parse_recursive("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");

    assert!(
        matches!(&tree, ParseTree::Pair(_, _)),
        "outer should be Pair (regex is Seq), got {:?}", tree
    );

    if let ParseTree::Pair(left, right) = &tree {
        assert!(
            matches!(left.as_ref(), ParseTree::Right(_)),
            "left factor should be Right (ab branch chosen), got {:?}", left
        );
        assert!(
            matches!(right.as_ref(), ParseTree::Right(_)),
            "right factor should be Right (ε branch chosen), got {:?}", right
        );
    }
}

#[test]
fn parse_recursive_paper_r1_on_a() {
    // "a" matches via (a)(ε) → Left(a), Right()
    let r = paper_r1();
    let tree = parse_recursive("a", &r).expect("should match");
    assert_round_trip(&tree, "a");
}

// ── Paper example: (a + b + ab)* on "ab" ───────────────────────────────────

#[test]
fn parse_recursive_paper_r2_on_ab() {
    // POSIX prefers the longer "ab" match in one star iteration over [a, b]
    let r = paper_r2();
    let tree = parse_recursive("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");

    // Should be a Star with exactly ONE iteration (the "ab" parse)
    if let ParseTree::Star(ref iters) = tree {
        assert_eq!(iters.len(), 1,
            "POSIX should pick 'ab' as one star iteration, got {:?}", iters);
    } else {
        panic!("expected Star, got {:?}", tree);
    }
}

// ============================================================================
// parse_loop — same results as parse_recursive (equivalence tests)
// ============================================================================

fn recursive_and_loop_agree(input: &str, r: &Regex) {
    let rec  = parse_recursive(input, r);
    let lp   = parse_loop(input, r);
    assert_parsers_agree("recursive", &rec, "loop", &lp);
}

#[test]
fn loop_agrees_on_eps()          { recursive_and_loop_agree("",    &Regex::Eps); }
#[test]
fn loop_agrees_on_literal()      { recursive_and_loop_agree("a",   &Regex::lit('a')); }
#[test]
fn loop_agrees_on_star_empty()   { recursive_and_loop_agree("",    &Regex::star(Regex::lit('a'))); }
#[test]
fn loop_agrees_on_star_three()   { recursive_and_loop_agree("aaa", &Regex::star(Regex::lit('a'))); }
#[test]
fn loop_agrees_on_no_match()     { recursive_and_loop_agree("b",   &Regex::lit('a')); }
#[test]
fn loop_agrees_on_paper_r1_ab()  { recursive_and_loop_agree("ab",  &paper_r1()); }
#[test]
fn loop_agrees_on_paper_r1_a()   { recursive_and_loop_agree("a",   &paper_r1()); }
#[test]
fn loop_agrees_on_paper_r2_ab()  { recursive_and_loop_agree("ab",  &paper_r2()); }
#[test]
fn loop_agrees_on_phi()          { recursive_and_loop_agree("a",   &Regex::Phi); }

#[test]
fn loop_agrees_on_complex_seq() {
    let r = Regex::seq(
        Regex::star(Regex::lit('a')),
        Regex::seq(Regex::lit('b'), Regex::star(Regex::lit('c'))),
    );
    for w in &["b", "ab", "bc", "abc", "aaabccc", "x"] {
        recursive_and_loop_agree(w, &r);
    }
}

// ============================================================================
// POSIX ordering verification (axioms A1/A2 from flops14 Definition 1)
// ============================================================================

#[test]
fn posix_a1_left_preferred_for_alt_when_lengths_equal() {
    // Alt(Eps, Eps) — both nullable, same length match → Left wins
    let r = Regex::alt(Regex::Eps, Regex::Eps);
    let tree = parse_recursive("", &r).expect("should match");
    assert!(matches!(tree, ParseTree::Left(_)),
        "POSIX A1: left alternative should be preferred, got {:?}", tree);
}

#[test]
fn posix_a2_longer_match_preferred_for_star() {
    // (a + ab)* on "ab" — POSIX prefers "ab" over ["a", "b"]
    // because POSIX maximises leftmost (longest first) per iteration
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))));
    let tree = parse_recursive("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");
    if let ParseTree::Star(ref iters) = tree {
        assert_eq!(iters.len(), 1,
            "POSIX: one long 'ab' iteration preferred over two short ones");
    } else {
        panic!("expected Star, got {:?}", tree);
    }
}

#[test]
fn posix_star_is_greedy_not_lazy() {
    // a* on "aaa" — POSIX must match all three 'a's, not stop early
    let r = Regex::star(Regex::lit('a'));
    let tree = parse_recursive("aaa", &r).expect("should match");
    if let ParseTree::Star(ref iters) = tree {
        assert_eq!(iters.len(), 3);
    } else {
        panic!("expected Star([a,a,a]), got {:?}", tree);
    }
}

// ============================================================================
// Tests for traced variants of parse_loop and parse_recursive
// ============================================================================

#[test]
fn recursive_traced_and_loop_traced_agree_on_all_paper_examples() {
    use regex_engine::posix::{parse_recursive_traced, parse_loop_traced};

    let cases: Vec<(&str, Regex)> = vec![
        ("aaa",  Regex::star(Regex::lit('a'))),
        ("aab",  Regex::star(Regex::lit('a'))),
        ("ab",   paper_r1()),
        ("a",    paper_r1()),
        ("ab",   paper_r2()),
        ("",     Regex::star(Regex::lit('a'))),
    ];

    for (input, r) in &cases {
        let (rec_tree, _)  = parse_recursive_traced(input, r);
        let (loop_tree, _) = parse_loop_traced(input, r);
        assert_eq!(
            rec_tree, loop_tree,
            "recursive_traced and loop_traced disagree on {:?}", input
        );
    }
}