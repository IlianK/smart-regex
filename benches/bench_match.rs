/*
Benchmarks for basic matchers (naive, deriv, pderiv)
*/

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::types::Regex;
use regex_engine::matchers::{match_naive, match_deriv, match_pderiv};

// ============================================================================
// Helpers
// ============================================================================

fn star_of_star() -> Regex {
    Regex::star(Regex::star(Regex::lit('a')))
}

fn ab_star() -> Regex {
    Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')))
}

fn repeated_seq(n: usize) -> Regex {
    let mut r = Regex::lit('a');
    for _ in 1..n {
        r = Regex::seq(r, Regex::lit('a'));
    }
    r
}

fn nested_star(n: usize) -> Regex {
    let mut r = Regex::star(Regex::lit('a'));
    for _ in 1..n {
        r = Regex::star(r);
    }
    r
}

fn repeat_char(c: char, n: usize) -> String {
    std::iter::repeat(c).take(n).collect()
}

// ============================================================================
// 1. Pathological patterns: (a*)* on "aaa...a"
// ============================================================================

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

    // Deriv and pderiv (larger inputs)
    for n in [5, 10, 50, 100, 500, 1000] {
        let _r = star_of_star();
        
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', n);
                let r = star_of_star();
                b.iter(|| match_deriv(black_box(&input), black_box(&r)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', n);
                let r = star_of_star();
                b.iter(|| match_pderiv(black_box(&input), black_box(&r)))
            },
        );
    }

    group.finish();
}


// ============================================================================
// 2. Benign patterns: (a|b)* on "ababab..."
// ============================================================================

fn bench_benign(c: &mut Criterion) {
    let mut group = c.benchmark_group("benign_(a|b)star");

    for n in [5, 10, 20, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("naive", n),
            &n,
            |b, &n| {
                let input: String = (0..n).map(|i| if i % 2 == 0 { 'a' } else { 'b' }).collect();
                let r = ab_star();
                b.iter(|| match_naive(black_box(&input), black_box(&r)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &n,
            |b, &n| {
                let input: String = (0..n).map(|i| if i % 2 == 0 { 'a' } else { 'b' }).collect();
                let r = ab_star();
                b.iter(|| match_deriv(black_box(&input), black_box(&r)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &n,
            |b, &n| {
                let input: String = (0..n).map(|i| if i % 2 == 0 { 'a' } else { 'b' }).collect();
                let r = ab_star();
                b.iter(|| match_pderiv(black_box(&input), black_box(&r)))
            },
        );
    }

    group.finish();
}


// ============================================================================
// 3. Nesting depth
// ============================================================================

fn bench_nesting(c: &mut Criterion) {
    let mut group = c.benchmark_group("nesting_depth");

    for n in 1..=4 {
        group.bench_with_input(
            BenchmarkId::new("naive", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', 6);
                let r = nested_star(n);
                b.iter(|| match_naive(black_box(&input), black_box(&r)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', 6);
                let r = nested_star(n);
                b.iter(|| match_deriv(black_box(&input), black_box(&r)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', 6);
                let r = nested_star(n);
                b.iter(|| match_pderiv(black_box(&input), black_box(&r)))
            },
        );
    }

    group.finish();
}


// ============================================================================
// 4. Sequence of 'a' repeated n times: a·a·a...·a
// ============================================================================

fn bench_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequence_a_repeated");

    for n in [3, 5, 8, 10] {
        group.bench_with_input(
            BenchmarkId::new("naive", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', n);
                let r = repeated_seq(n);
                b.iter(|| match_naive(black_box(&input), black_box(&r)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("deriv", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', n);
                let r = repeated_seq(n);
                b.iter(|| match_deriv(black_box(&input), black_box(&r)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("pderiv", n),
            &n,
            |b, &n| {
                let input = repeat_char('a', n);
                let r = repeated_seq(n);
                b.iter(|| match_pderiv(black_box(&input), black_box(&r)))
            },
        );
    }

    group.finish();
}


// ============================================================================
// Register all benchmark groups
// ============================================================================

criterion_group!(
    benches,
    bench_pathological,
    bench_benign,
    bench_nesting,
    bench_sequence,
);
criterion_main!(benches);