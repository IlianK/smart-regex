//! regex-engine/benches/bench_posix.rs
//!
//! Benchmarks for the four parser combinations (3x posix + 1x greedy) :
//!   - recursive  (standard/deriv, two-pass, native recursion)
//!   - loop       (standard/deriv, two-pass, explicit Vec instead of recursion)
//!   - bitcoded   (bitcoded/deriv, single fused forward pass)
//!   - pderiv_bc  (bitcoded/pderiv, single forward pass over a residual set (greedy))

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::parsers::{parse_bitcoded, parse_loop, parse_pderiv_bc, parse_recursive};
use regex_engine::types::Regex;

// Helpers
fn star_a() -> Regex {
    Regex::star(Regex::lit('a'))
}

fn deep_sequence(n: usize) -> Regex {
    let mut r = Regex::lit('a');
    for _ in 1..n {
        r = Regex::seq(r, Regex::lit('a'));
    }
    r
}

fn repeat_char(c: char, n: usize) -> String {
    std::iter::repeat(c).take(n).collect()
}


// 1. Small patterns
fn bench_small_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_patterns");
    group.sample_size(100);

    let test_cases: Vec<(&str, Regex, &str)> = vec![
        ("literal_a", Regex::lit('a'), "a"),
        ("star_a_empty", star_a(), ""),
        ("star_a_one", star_a(), "a"),
        ("star_a_three", star_a(), "aaa"),
    ];

    for (name, regex, input) in test_cases {
        group.bench_with_input(
            BenchmarkId::new("recursive", name),
            &(input, regex.clone()),
            |b, (input, r)| b.iter(|| parse_recursive(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("loop", name),
            &(input, regex.clone()),
            |b, (input, r)| b.iter(|| parse_loop(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("bitcoded", name),
            &(input, regex.clone()),
            |b, (input, r)| b.iter(|| parse_bitcoded(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("pderiv_bc", name),
            &(input, regex),
            |b, (input, r)| b.iter(|| parse_pderiv_bc(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// 2. Scaling: a* on "aaa...a" (shallow expression)
fn bench_scaling_a_star(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_a_star");
    let r = star_a();

    for n in [10, 50, 100, 200, 400, 600, 800, 1000] {
        let input = repeat_char('a', n);

        group.bench_with_input(
            BenchmarkId::new("recursive", n),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_recursive(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("loop", n),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_loop(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("bitcoded", n),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_bitcoded(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("pderiv_bc", n),
            &(input, r.clone()),
            |b, (input, r)| b.iter(|| parse_pderiv_bc(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// 3. Scaling: deep expression (a·a·a...·a)
fn bench_deep_expression(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_expression");

    for depth in [50, 100, 200, 400, 600, 800, 1000] {
        let r = deep_sequence(depth);
        let input = repeat_char('a', depth);

        group.bench_with_input(
            BenchmarkId::new("recursive", depth),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_recursive(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("loop", depth),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_loop(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("bitcoded", depth),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_bitcoded(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("pderiv_bc", depth),
            &(input, r.clone()),
            |b, (input, r)| b.iter(|| parse_pderiv_bc(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// 4. Ambiguous/POSIX-vs-Greedy-relevant pattern: (a+aa)* on "aaaa...a"
//    -- stresses the axis Section 7.5 (deriv-vs-pderiv-performance) discusses:
//    cost of maintaining a set of residuals (pderiv_bc) vs. a single
//    residual (recursive/loop/bitcoded), independent of which answer
//    each construction settles on.
fn bench_ambiguous_star(c: &mut Criterion) {
    let mut group = c.benchmark_group("ambiguous_star_a_or_aa");

    let r = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('a')),
    ));

    for n in [10, 50, 100, 200, 400] {
        let input = repeat_char('a', n);

        group.bench_with_input(
            BenchmarkId::new("recursive", n),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_recursive(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("bitcoded", n),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| parse_bitcoded(black_box(input), black_box(r))),
        );

        group.bench_with_input(
            BenchmarkId::new("pderiv_bc", n),
            &(input, r.clone()),
            |b, (input, r)| b.iter(|| parse_pderiv_bc(black_box(input), black_box(r))),
        );
    }

    group.finish();
}


// Register all benchmark groups
criterion_group!(
    benches,
    bench_small_patterns,
    bench_scaling_a_star,
    bench_deep_expression,
    bench_ambiguous_star,
);

criterion_main!(benches);
