# Benchmarks

Five `cargo bench` targets: `bench_match`/`bench_parse` against small
hand-constructed patterns, `bench_dataset`/`bench_external`/`bench_memory`
against the real-world corpus (see [DATASETS.md](DATASETS.md)).
`bench_match`, `bench_parse`, `bench_dataset`, and `bench_external` use
Criterion.rs; append `-- --test` to any of their commands below to run
each benchmark function once for a quick correctness check instead of
the full statistical sampling run. `bench_memory` is a plain reporting
binary, not a Criterion target -- see its own section for why.

## Selecting a category, source, or sample size

`bench_dataset`, `bench_external`, and `bench_memory` all read the same
three environment variables, so a run can be narrowed without editing
and recompiling. Unset means "all" for `BENCH_CATEGORY`/`BENCH_SOURCE`;
`criterion_main!` already owns `cargo bench`'s own CLI flags, which is
why this is env vars rather than a custom `--category` flag.

| Variable | Values | Default |
|---|---|---|
| `BENCH_CATEGORY` | `best`, `neutral`, `worst` | unset (all three) |
| `BENCH_SOURCE` | comma-separated subset of `suricata`, `spamassassin`, `regexlib` | unset (all three) |
| `BENCH_PATTERN_LIMIT` | positive integer | `30` |

```
# Just the Worst category, just SpamAssassin, capped at 10 distinct patterns
BENCH_CATEGORY=worst BENCH_SOURCE=spamassassin BENCH_PATTERN_LIMIT=10 cargo bench --bench bench_dataset
```

A filter that leaves nothing loaded panics with the same per-source
diagnosis as an empty corpus, plus the active filter values, so a typo'd
category or a source with nothing prepared is never silently a no-op.

## `bench_match.rs`

```
cargo bench --bench bench_match
```

Benchmarks the three Boolean matchers (`match_naive`, `match_deriv`,
`match_pderiv`) against four synthetic pattern families: `pathological`
($(a^{*})^{*}$, where `match_naive` is expected to blow up),
`benign` ($(a+b)^{*}$), `nesting_depth` ($a^{*}$ nested 1-4 times), and
`sequence_a_repeated` (a flat `a·a·...·a` chain). No parse tree involved
here, membership only.

## `bench_parse.rs`

```
cargo bench --bench bench_parse
```

Benchmarks all five parsers (`deriv_std_rec`, `deriv_std_loop`,
`deriv_bc`, `pderiv_std`, `pderiv_bc`) against four synthetic families:
`small_patterns` (fixed per-call overhead), `scaling_a_star` (cost as a
function of input length alone), `deep_expression` (cost as a function
of expression size), and `ambiguous_star_a_or_aa` (cost from genuine
ambiguity, $(a+aa)^{*}$, where `deriv_bc`'s bit-carrying branches can't
collapse the way `simp` collapses unambiguous ones).

## `bench_dataset.rs`

```
cargo run --release --example prepare_dataset -- <source> <file>...  # once per source first
cargo bench --bench bench_dataset
```

Benchmarks all five parsers against the real-world corpus, grouped by
`Category` (`dataset_best`, `dataset_neutral`, `dataset_worst`), each
pattern probed with its own generated and independently verified
input(s), not a string shared across the whole group. The loader admits
up to 30 distinct *patterns* per category, not rows: every verified
variant (up to `--variants`, default 5, from `prepare_dataset`) of an
admitted pattern is included. A separate group,
`dataset_worst_per_pattern`, isolates up to five individual worst-case
patterns instead, one measurement per pattern rather than summed.
Panics with a per-source diagnosis (resolved path, each source's own
line count) if nothing loads, rather than silently benchmarking an empty
corpus.

## `bench_external.rs`

```
cargo bench --bench bench_external --features external-engines
```

Needs the system RE2 library at build time (`external-engines` links
against it via `build.rs`/pkg-config); off by default. Runs the same
per-category corpus (`external_best`/`neutral`/`worst`) through all five
internal parsers plus Rust's `regex` crate and Google's RE2 (both Perl
and POSIX syntax modes), and separately through the crate's own
tree-free matchers (`external_matcher_*`) against the same three
engines. `external_agreement_smoke` is a one-off check, not a timing
comparison: how often this crate's own membership answer agrees with
`regex` and with RE2, against each entry's independently verified
expected outcome.

## `bench_memory.rs`

```
cargo bench --bench bench_memory
```

Bytes allocated per parser per category, the memory counterpart to
`bench_dataset.rs`'s timing. Not a Criterion target: allocation counts
are deterministic per input, not noisy the way wall-clock time is, so
repeated statistical sampling would only measure Criterion's own
overhead, not anything about the parsers. `harness = false` in
`Cargo.toml` gives it a plain `fn main()` instead of `criterion_main!`,
installs a counting `#[global_allocator]` (wraps the system allocator,
tracks bytes via atomics, doesn't change what actually gets allocated),
and prints one table per category: total bytes allocated per parser
(summed across every entry, including freed-and-reallocated churn),
average per entry, and the largest single entry's peak live-byte
high-water mark. Verified live (`BENCH_PATTERN_LIMIT=3`, all sources):
`deriv_bc`/`pderiv_bc`/`pderiv_std` allocate roughly 37-39% of what
`deriv_std_rec`/`deriv_std_loop` do in total bytes across all three
categories on this corpus, the real-world counterpart to Section 8's
"Heap allocation" argument (`deriv_std` keeps every expression in its
derivation sequence alive at once; `deriv_bc` reassigns one `ARegex` in
place per step).
