// Integration tests for parsers/deriv_std/: parse_recursive and parse_loop. Paper examples from flops14-extended.

mod common;
use common::{assert_round_trip, assert_parsers_agree, paper_r1, paper_r2};

use regex_engine::{Regex, ParseTree};
use regex_engine::parsers::{parse_deriv_std_rec, parse_deriv_std_loop};

// parse_recursive

#[test]
fn parse_recursive_empty_word_on_eps() {
    let r = Regex::Eps;
    let tree = parse_deriv_std_rec("", &r).expect("should match");
    assert_eq!(tree, ParseTree::Empty);
}

#[test]
fn parse_recursive_single_literal() {
    let r = Regex::lit('a');
    let tree = parse_deriv_std_rec("a", &r).expect("should match");
    assert_eq!(tree, ParseTree::Char('a'));
    assert_round_trip(&tree, "a");
}

#[test]
fn parse_recursive_sequence() {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    let tree = parse_deriv_std_rec("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");
    assert!(matches!(tree, ParseTree::Pair(_, _)));
}

#[test]
fn parse_recursive_star_empty() {
    let r = Regex::star(Regex::lit('a'));
    let tree = parse_deriv_std_rec("", &r).expect("should match");
    assert_eq!(tree, ParseTree::Star(vec![]));
}

#[test]
fn parse_recursive_star_three_chars() {
    let r = Regex::star(Regex::lit('a'));
    let tree = parse_deriv_std_rec("aaa", &r).expect("should match");
    assert_round_trip(&tree, "aaa");
    assert!(matches!(&tree, ParseTree::Star(v) if v.len() == 3));
}

#[test]
fn parse_recursive_returns_none_on_no_match() {
    let r = Regex::lit('a');
    assert!(parse_deriv_std_rec("b", &r).is_none());
    assert!(parse_deriv_std_rec("", &r).is_none());
}

#[test]
fn parse_recursive_phi_never_matches() {
    assert!(parse_deriv_std_rec("",  &Regex::Phi).is_none());
    assert!(parse_deriv_std_rec("a", &Regex::Phi).is_none());
}

// Paper examples (flops14-extended)

/// (a + ab)(b + eps) on "ab": A1 picks the longer "ab" branch on the left factor.
#[test]
fn parse_recursive_paper_r1_on_ab() {
    let r = paper_r1();
    let tree = parse_deriv_std_rec("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");

    assert!(
        matches!(&tree, ParseTree::Pair(_, _)),
        "outer should be Pair (regex is Seq), got {:?}", tree
    );

    if let ParseTree::Pair(left, right) = &tree {
        assert!(
            matches!(left.as_ref(), ParseTree::Right(_)),
            "left factor should be Right (ab branch chosen by A1), got {:?}", left
        );
        assert!(
            matches!(right.as_ref(), ParseTree::Right(_)),
            "right factor should be Right (ε branch, only valid choice), got {:?}", right
        );
    }
}

/// (a + ab)(b + eps) on "a": only the "a" branch is even valid, no A1/A2 ambiguity.
#[test]
fn parse_recursive_paper_r1_on_a() {
    let r = paper_r1();
    let tree = parse_deriv_std_rec("a", &r).expect("should match");
    assert_round_trip(&tree, "a");

    if let ParseTree::Pair(left, right) = &tree {
        assert!(
            matches!(left.as_ref(), ParseTree::Left(_)),
            "left factor should be Left (only 'a' branch valid), got {:?}", left
        );
        assert!(
            matches!(right.as_ref(), ParseTree::Right(_)),
            "right factor should be Right (ε branch, only valid choice), got {:?}", right
        );
    } else {
        panic!("expected Pair, got {:?}", tree);
    }
}

/// (a + b + ab)* on "ab": A1 picks one "ab" iteration over Greedy's two-iteration answer.
#[test]
fn parse_recursive_paper_r2_on_ab() {
    let r = paper_r2();
    let tree = parse_deriv_std_rec("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");

    if let ParseTree::Star(ref iters) = tree {
        assert_eq!(iters.len(), 1,
            "POSIX should pick 'ab' as one star iteration (A1), got {:?}", iters);

        assert!(
            matches!(&iters[0], ParseTree::Right(inner)
                if matches!(inner.as_ref(), ParseTree::Right(innermost)
                    if matches!(innermost.as_ref(), ParseTree::Pair(_, _)))),
            "iteration should be Right(Right(Pair(a,b))), got {:?}", iters[0]
        );
    } else {
        panic!("expected Star, got {:?}", tree);
    }
}

// parse_loop

fn recursive_and_loop_agree(input: &str, r: &Regex) {
    let rec = parse_deriv_std_rec(input, r);
    let lp  = parse_deriv_std_loop(input, r);
    assert_parsers_agree("recursive", &rec, "loop", &lp);
}

#[test]
fn loop_agrees_on_eps()         { recursive_and_loop_agree("",    &Regex::Eps); }
#[test]
fn loop_agrees_on_literal()     { recursive_and_loop_agree("a",   &Regex::lit('a')); }
#[test]
fn loop_agrees_on_star_empty()  { recursive_and_loop_agree("",    &Regex::star(Regex::lit('a'))); }
#[test]
fn loop_agrees_on_star_three()  { recursive_and_loop_agree("aaa", &Regex::star(Regex::lit('a'))); }
#[test]
fn loop_agrees_on_no_match()    { recursive_and_loop_agree("b",   &Regex::lit('a')); }
#[test]
fn loop_agrees_on_paper_r1_ab() { recursive_and_loop_agree("ab",  &paper_r1()); }
#[test]
fn loop_agrees_on_paper_r1_a()  { recursive_and_loop_agree("a",   &paper_r1()); }
#[test]
fn loop_agrees_on_paper_r2_ab() { recursive_and_loop_agree("ab",  &paper_r2()); }
#[test]
fn loop_agrees_on_phi()         { recursive_and_loop_agree("a",   &Regex::Phi); }

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

// POSIX ordering rules (flops14 Definition 1)

/// A2: Alt(Eps, Eps) on "" -- equal-length match, left wins.
#[test]
fn posix_a2_left_preferred_when_lengths_equal() {
    let r = Regex::alt(Regex::Eps, Regex::Eps);
    let tree = parse_deriv_std_rec("", &r).expect("should match");
    assert!(
        matches!(tree, ParseTree::Left(_)),
        "POSIX A2: left alternative preferred when lengths equal, got {:?}", tree
    );
}

/// A1: (a + ab)* on "ab" -- one long "ab" iteration preferred over two short "a" iterations.
#[test]
fn posix_a1_longer_match_preferred_for_star() {
    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
    ));
    let tree = parse_deriv_std_rec("ab", &r).expect("should match");
    assert_round_trip(&tree, "ab");
    if let ParseTree::Star(ref iters) = tree {
        assert_eq!(iters.len(), 1,
            "POSIX A1: one long 'ab' iteration preferred over two short 'a' iterations");
    } else {
        panic!("expected Star, got {:?}", tree);
    }
}

/// K1: eps* on "" -- zero iterations, Star([]), never loops forever.
#[test]
fn posix_k1_star_of_eps_is_empty() {
    let r = Regex::star(Regex::Eps);
    let tree = parse_deriv_std_rec("", &r).expect("should match");
    assert_eq!(tree, ParseTree::Star(vec![]),
        "POSIX K1: ε* must produce Star([]), got {:?}", tree);
}

/// K2: (eps+a)* on "a" -- the non-empty iteration is preferred over the empty one.
#[test]
fn posix_k2_nonempty_preferred_over_empty_in_star() {
    let r = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    let tree = parse_deriv_std_rec("a", &r).expect("should match");
    assert_round_trip(&tree, "a");
    if let ParseTree::Star(ref iters) = tree {
        assert_eq!(iters.len(), 1,
            "POSIX K2: one non-empty iteration preferred, got {:?}", iters);
        assert!(
            matches!(&iters[0], ParseTree::Right(_)),
            "POSIX K2: right branch (non-empty 'a') should be chosen, got {:?}", iters[0]
        );
    } else {
        panic!("expected Star, got {:?}", tree);
    }
}

// Traced test

#[test]
fn recursive_traced_and_loop_traced_agree_on_all_paper_examples() {
    use regex_engine::parsers::{parse_recursive_traced, parse_loop_traced};

    let cases: Vec<(&str, Regex)> = vec![
        ("aaa", Regex::star(Regex::lit('a'))),
        ("aab", Regex::star(Regex::lit('a'))),
        ("ab",  paper_r1()),
        ("a",   paper_r1()),
        ("ab",  paper_r2()),
        ("",    Regex::star(Regex::lit('a'))),
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
