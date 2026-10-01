# Benchmarks

Five `cargo bench` targets are provided:

- `bench_match` / `bench_parse`: synthetic, hand-constructed patterns.
- `bench_dataset` / `bench_external` / `bench_memory`: real-world corpus
  (see [DATASETS.md](DATASETS.md)).

The first four use Criterion.rs; append `-- --test` to run each benchmark
function once as a quick correctness check. `bench_memory` is a plain
reporting binary rather than a Criterion target.

## Selecting a category, source, or sample size

`bench_dataset`, `bench_external`, and `bench_memory` support the same
environment variables:

| Variable | Values | Default |
|---|---|---|
| `BENCH_CATEGORY` | `best`, `neutral`, `worst` | all |
| `BENCH_SOURCE` | comma-separated `suricata`, `spamassassin`, `regexlib` | all |
| `BENCH_PATTERN_LIMIT` | positive integer | `30` |

These allow runs to be narrowed without recompiling. For example:

    BENCH_CATEGORY=worst BENCH_SOURCE=spamassassin BENCH_PATTERN_LIMIT=10 \
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

    cargo run --release --example prepare_dataset -- <source> <file>...
    cargo bench --bench bench_dataset

Benchmarks all five parsers on the real-world corpus, grouped into
`dataset_best`, `dataset_neutral`, and `dataset_worst`. Each pattern is tested
against its own independently generated and verified input variants.

Up to 30 distinct **patterns** per category are admitted; each contributes
all verified variants (up to `--variants`, default 5). The separate
`dataset_worst_per_pattern` group measures up to five worst-case patterns
individually rather than as a combined group.

An empty result produces a per-source diagnostic instead of silently measuring
an empty corpus.

## `bench_external.rs`

    cargo bench --bench bench_external --features external-engines

Requires the system RE2 library and is disabled by default. It runs the same
per-category corpus through:

- all five internal parsers vs. Rust's `regex` crate and RE2;
- the crate's tree-free matchers vs. the same three external engines;
- `external_agreement_smoke`, a non-timed check comparing this crate's
  membership results with `regex` and RE2 against the independently verified
  expected outcomes.

See [EXTERNAL_ENGINES.md](EXTERNAL_ENGINES.md) for the interpretation and
limitations of these comparisons, including why `re2_posix` totals are not
directly comparable and why `external_matcher_*` is the more like-for-like
speed comparison.

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