/*
Benchmarks for POSIX parsers: Recursive vs Loop
*/

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::types::Regex;
use regex_engine::posix::{parse_recursive, parse_loop};

// ============================================================================
// Helpers
// ============================================================================

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


// ============================================================================
// 1. Small patterns
// ============================================================================

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
            &(input, regex),
            |b, (input, r)| b.iter(|| parse_loop(black_box(input), black_box(r))),
        );
    }
    
    group.finish();
}


// ============================================================================
// 2. Scaling: a* on "aaa...a" (shallow expression)
// ============================================================================

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
            &(input, r.clone()),
            |b, (input, r)| b.iter(|| parse_loop(black_box(input), black_box(r))),
        );
    }
    
    group.finish();
}


// ============================================================================
// 3. Scaling: deep expression (a·a·a...·a)
// ============================================================================

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
            &(input, r.clone()),
            |b, (input, r)| b.iter(|| parse_loop(black_box(input), black_box(r))),
        );
    }
    
    group.finish();
}


// ============================================================================
// Register all benchmark groups
// ============================================================================

criterion_group!(
    benches,
    bench_small_patterns,
    bench_scaling_a_star,
    bench_deep_expression,
);

criterion_main!(benches);