# Benchmarks

Criterion benchmarks under `benches/`.

```bash
# Run all benchmarks (HTML report: target/criterion/report/index.html)
cargo bench
cargo bench -- --verbose

# Specific benchmark file
cargo bench --bench bench_match
cargo bench --bench bench_posix

# Specific group within a file
cargo bench --bench bench_posix -- scaling_a_star
cargo bench --bench bench_posix -- ambiguous_star_a_or_aa
```

| File | Groups | Covers |
|---|---|---|
| `bench_match.rs` | `pathological_(a*)star`, `benign_(a\|b)star`, `nesting_depth`, `sequence_a_repeated` | The three matchers: `match_naive`, `match_deriv`, `match_pderiv` |
| `bench_posix.rs` | `small_patterns`, `scaling_a_star`, `deep_expression`, `ambiguous_star_a_or_aa` | All four parser combinations: `parse_recursive`, `parse_loop`, `parse_bitcoded`, `parse_pderiv_bc` |
