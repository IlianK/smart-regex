/*
Benchmarks for POSIX parsers: Recursive vs Loop
*/

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex_engine::basic::Regex;
use regex_engine::posix::{parse_recursive, parse_loop};

// ============================================================================
// Helpers
// ============================================================================

fn star_a() -> Regex {
    Regex::star(Regex::lit('a'))
}

fn epsilon_alt_a_star() -> Regex {
    Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')))
}

fn ab_alt_star() -> Regex {
    Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')))
}

fn repeat_char(c: char, n: usize) -> String {
    std::iter::repeat(c).take(n).collect()
}

// ============================================================================
// 1. Small patterns (baseline overhead)
// ============================================================================

fn bench_small_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_patterns");
    group.sample_size(100);
    
    let test_cases: Vec<(&str, Regex, &str)> = vec![
        ("literal_a", Regex::lit('a'), "a"),
        ("star_a_empty", star_a(), ""),
        ("star_a_one", star_a(), "a"),
        ("star_a_three", star_a(), "aaa"),
        ("epsilon_alt_a", epsilon_alt_a_star(), "a"),
        ("ab_alt", ab_alt_star(), "ab"),
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
// 2. Scaling: a* on "aaa...a"
// ============================================================================

fn bench_scaling_a_star(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_a_star");
    let r = star_a();
    
    for n in [10, 50, 100, 500, 1000, 2000, 5000, 10000] {
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
// 3. Scaling: (ε + a)* on "aaa...a" (tests ε handling)
// ============================================================================

fn bench_scaling_epsilon_alt_star(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_epsilon_alt_star");
    let r = epsilon_alt_a_star();
    
    for n in [10, 50, 100, 500, 1000, 2000, 5000] {
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
// 4. POSIX decision logic (ambiguous patterns)
// ============================================================================

fn bench_ambiguous_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("ambiguous_patterns");
    
    let test_cases: Vec<(&str, Regex, &str)> = vec![
        (
            "ab_vs_a_b",
            Regex::star(Regex::alt(
                Regex::seq(Regex::lit('a'), Regex::lit('b')),
                Regex::alt(Regex::lit('a'), Regex::lit('b'))
            )),
            "ab",
        ),
        (
            "a_vs_ab_right",
            Regex::star(Regex::alt(
                Regex::lit('a'),
                Regex::seq(Regex::lit('a'), Regex::lit('b'))
            )),
            "ab",
        ),
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
// 5. Stack depth 
// ============================================================================

fn bench_stack_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("stack_depth");
    let r = star_a();
    
    // Loop handles all lengths. Recursive will crash somewhere above 5000-10000.
    for n in [1000, 2000, 5000, 10000] {
        let input = repeat_char('a', n);
        
        group.bench_with_input(
            BenchmarkId::new("recursive", n),
            &(input.clone(), r.clone()),
            |b, (input, r)| b.iter(|| {
                let _ = parse_recursive(black_box(input), black_box(r));
            }),
        );
        
        group.bench_with_input(
            BenchmarkId::new("loop", n),
            &(input, r.clone()),
            |b, (input, r)| b.iter(|| {
                let _ = parse_loop(black_box(input), black_box(r));
            }),
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
    bench_scaling_epsilon_alt_star,
    bench_ambiguous_patterns,
    bench_stack_depth,
);

criterion_main!(benches);