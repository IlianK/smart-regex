# Benchmarks

Four `cargo bench` targets are provided:

- `bench_match` / `bench_parse`: synthetic, hand-constructed patterns.
- `bench_dataset` / `bench_memory`: real-world corpus
  (see [DATASET.md](DATASET.md)).

The first three use Criterion.rs; append `-- --test` to run each benchmark
function once as a quick correctness check. `bench_memory` is a plain
reporting binary rather than a Criterion target.

## Selecting a source or sample size

`bench_dataset` and `bench_memory` support the same
environment variables:

| Variable | Values | Default |
|---|---|---|
| `BENCH_SOURCE` | comma-separated `suricata`, `spamassassin`, `regexlib` | all |
| `BENCH_PATTERN_LIMIT` | positive integer | `30` |

These allow runs to be narrowed without recompiling. For example:

    BENCH_SOURCE=spamassassin BENCH_PATTERN_LIMIT=10 \
      cargo bench --bench bench_dataset

If filtering loads nothing, the benchmark panics with a per-source diagnosis
and the active filters rather than silently running an empty benchmark.

## `bench_match.rs`

    cargo bench --bench bench_match

Benchmarks `match_naive`, `match_deriv`, and `match_pderiv` on four synthetic
families: `pathological` (`(a*)*`), `benign` (`(a+b)*`), `nesting_depth`
(`a*` nested 1–4 times), and `sequence_a_repeated` (a flat `a·a·...·a`
chain). This measures boolean membership only; no parse tree is involved.

## `bench_parse.rs`

    cargo bench --bench bench_parse

Benchmarks all five parsers (`deriv_std_rec`, `deriv_std_loop`, `deriv_bc`,
`pderiv_std`, `pderiv_bc`) on four families:

- `small_patterns`: fixed per-call overhead.
- `scaling_a_star`: cost as a function of input length.
- `deep_expression`: cost as a function of expression size.
- `ambiguous_star_a_or_aa`: genuine ambiguity from `(a+aa)*`.

## `bench_dataset.rs`

    cargo run --release --example dataset_prepare -- <source> <file>...
    cargo bench --bench bench_dataset

Two Criterion groups:

- `dataset_<source>` (`dataset_suricata`, `dataset_spamassassin`,
  `dataset_regexlib`): all five parsers over that source's whole loaded
  corpus, one combined throughput measurement per parser.
- `dataset_per_pattern`: every loaded source combined, five patterns per
  source, each parser timed on each individual (pattern, input) entry
  separately.

Up to `BENCH_PATTERN_LIMIT` distinct patterns per source are admitted into
`dataset_<source>`; each contributes all of its verified variants (up to
`--variants`, default 5, from `dataset_prepare`).

An empty result produces a per-source diagnostic instead of silently measuring
an empty corpus.

## `bench_memory.rs`

    cargo bench --bench bench_memory

Reports bytes allocated per parser and category as the memory counterpart to
`bench_dataset`. It is not a Criterion target: allocation counts are
deterministic for a given input, so repeated statistical sampling would mainly
measure Criterion overhead.

With `harness = false`, it uses a counting `#[global_allocator]` that wraps
the system allocator and tracks allocated bytes. For each category it reports
total allocated bytes (including freed-and-reallocated churn), average bytes
per entry, and the largest entry's peak live-byte high-water mark.

On the verified live corpus (`BENCH_PATTERN_LIMIT=3`, all sources),
`deriv_bc`, `pderiv_bc`, and `pderiv_std` allocate roughly 37–39% as many
total bytes as `deriv_std_rec` and `deriv_std_loop`. This corresponds to the
heap-allocation difference described in Section 8: `deriv_std` retains its
derivation sequence, whereas `deriv_bc` reassigns one `ARegex` in place.