/// benches/matcher_bench.rs

/*
Benchmarks for the three regex matchers with 4 groups
    1. Pathological input  -> (a*)* on "aaa...a", classic ReDoS shape
    2. Scaling by input    -> fixed expression, growing input length
    3. Scaling by nesting  -> fixed input, growing expression complexity
    4. Sequence matching   -> a.a.a...a on "aaa...a", also exponential for naive
*/

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::{match_deriv, match_naive, match_pderiv, Regex};


// ---------------------------------------------
// Helpers for expression building
// ---------------------------------------------

// (a*)* -> pathological expression for naive backtracking
// Every additional 'a' in input multiplies naive matcher's work
fn star_of_star() -> Regex {
    Regex::star(Regex::star(Regex::lit('a')))
}

// (a|b)* -> benign expression, all three matchers should be fast
fn ab_star() -> Regex {
    Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')))
}

// a . a . a ... (n times) -> deeply nested sequence
fn repeated_seq(n: usize) -> Regex {
    assert!(n >= 1);
    let mut r = Regex::lit('a');
    for _ in 1..n {
        r = Regex::seq(r, Regex::lit('a'));
    }
    r
}

// (a*)* nested n times -> escalating complexity (n=1: a* -> n=2: (a*)* -> n=3: ((a*)*)*)
fn nested_star(n: usize) -> Regex {
    let mut r = Regex::star(Regex::lit('a'));
    for _ in 1..n {
        r = Regex::star(r);
    }
    r
}

// Build input string of n repetitions of given char
fn repeat_char(c: char, n: usize) -> String {
    std::iter::repeat(c).take(n).collect()
}


// ---------------------------------------------
// Pathological input: (a*)* on "aaa...a"
// ---------------------------------------------

// Naive matcher explores exponentially many splits (ReDos pattern)
// Deriv and PDeriv are linear regardless of input length
//
// -> Naive is capped at <=15 because it becomes too slow beyond that
// -> Deriv and PDeriv are tested up to much larger inputs to show linear scaling
fn bench_pathological(c: &mut Criterion) {
    let mut group = c.benchmark_group("pathological_(a*)star");

    // Naive matcher (small inputs only)
    for n in [5, 8, 10, 12, 15] {
        let input = repeat_char('a', n);
        let r = star_of_star();
        group.bench_with_input(
            BenchmarkId::new("naive", n),
            &(input, r),
            |b, (input, r)| b.iter(|| match_naive(black_box(input), black_box(r))),
        );
    }

    // Brzozowski (scales linearly)
    for n in [5, 10, 50, 100, 500, 1000] {
        let input = repeat_char('a', n);
        let r = star_of_star();
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &(input, r),
            |b, (input, r)| b.iter(|| match_deriv(black_box(input), black_box(r))),
        );
    }

    // Antimirov (scales linearly)
    for n in [5, 10, 50, 100, 500, 1000] {
        let input = repeat_char('a', n);
        let r = star_of_star();
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &(input, r),
            |b, (input, r)| b.iter(|| match_pderiv(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// ---------------------------------------------
// Benign expression: (a|b)* on "abab...ab"
// ---------------------------------------------

// Expected, that all three matchers should scale similarly 
// -> Naive matcher is not always bad 
// -> Only pathological patterns trigger blowup
fn bench_benign(c: &mut Criterion) {
    let mut group = c.benchmark_group("benign_(a|b)star");

    for n in [5, 10, 20, 50, 100] {
        // Alternate a and b for a realistic input
        let input: String = (0..n).map(|i| if i % 2 == 0 { 'a' } else { 'b' }).collect();

        let r_naive  = ab_star();
        let r_deriv  = ab_star();
        let r_pderiv = ab_star();

        // Naive Matcher
        group.bench_with_input(
            BenchmarkId::new("naive", n),
            &(input.clone(), r_naive),
            |b, (input, r)| b.iter(|| match_naive(black_box(input), black_box(r))),
        );

        // Brzozowski Matcher
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &(input.clone(), r_deriv),
            |b, (input, r)| b.iter(|| match_deriv(black_box(input), black_box(r))),
        );

        // Antimirov Matcher
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &(input.clone(), r_pderiv),
            |b, (input, r)| b.iter(|| match_pderiv(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// ---------------------------------------------
// Scaling by nesting depth
// ---------------------------------------------

// Fixed input "aaa" (3 chars), increasing nesting:
//   n=1: a*          on "aaa"
//   n=2: (a*)*       on "aaa"
//   n=3: ((a*)*)*    on "aaa"
// -> To show how nesting alone drives naive complexity up

fn bench_nesting(c: &mut Criterion) {
    let mut group = c.benchmark_group("nesting_depth");
    let input = repeat_char('a', 6); // fixed short input

    for n in 1..=4 {
        let r_naive  = nested_star(n);
        let r_deriv  = nested_star(n);
        let r_pderiv = nested_star(n);

        // Naive Matcher
        group.bench_with_input(
            BenchmarkId::new("naive", n),
            &(input.clone(), r_naive),
            |b, (input, r)| b.iter(|| match_naive(black_box(input), black_box(r))),
        );

        // Brzozowski Matcher
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &(input.clone(), r_deriv),
            |b, (input, r)| b.iter(|| match_deriv(black_box(input), black_box(r))),
        );

        // Antimirov Matcher
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &(input.clone(), r_pderiv),
            |b, (input, r)| b.iter(|| match_pderiv(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// ---------------------------------------------
// Sequence matching
// ---------------------------------------------

// a.a.a...a (n literals) matched against "aaa...a"
// Naive splits every possible way -> also exponential
// -> To show that the problem is not limited to Star
fn bench_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequence_a_repeated");

    for n in [3, 5, 8, 10] {
        let input = repeat_char('a', n);
        let r_naive  = repeated_seq(n);
        let r_deriv  = repeated_seq(n);
        let r_pderiv = repeated_seq(n);

        // Naive Matcher
        group.bench_with_input(
            BenchmarkId::new("naive", n),
            &(input.clone(), r_naive),
            |b, (input, r)| b.iter(|| match_naive(black_box(input), black_box(r))),
        );

        // Brzozowski Matcher
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &(input.clone(), r_deriv),
            |b, (input, r)| b.iter(|| match_deriv(black_box(input), black_box(r))),
        );

        // Antimirov Matcher
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &(input.clone(), r_pderiv),
            |b, (input, r)| b.iter(|| match_pderiv(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// ---------------------------------------------
// Register groups
// ---------------------------------------------

// Run all benchmarks together (reports in target/criterion)
criterion_group!(
    benches,
    bench_pathological,
    bench_benign,
    bench_nesting,
    bench_sequence,
);
criterion_main!(benches);