# Benchmarks

Four `cargo bench` targets are provided:

- `bench_match` / `bench_parse`: synthetic, hand-constructed patterns.
- `bench_dataset` / `bench_memory`: real-world corpus
  (see [DATASET.md](DATASET.md)).

The first three use Criterion.rs; append `-- --test` to run each benchmark
function once as a quick correctness check. `bench_memory` is a plain
reporting binary rather than a Criterion target.



## Bench Match & Parse

Both targets below measure the same stack on synthetic, hand-constructed 
inputs rather than the real corpus (`bench_dataset` / `bench_memory` cover that).


### `bench_match.rs`

    cargo bench --bench bench_match
    BENCH_SIZE_LIMIT=100 cargo bench --bench bench_match
    BENCH_SAMPLE_SIZE=50 cargo bench --bench bench_match

Benchmarks `match_naive`, `match_deriv`, and `match_pderiv` on four synthetic
families: `pathological` (`(a*)*`), `benign` (`(a|b)*`), `nesting_depth`
(`a*` nested 1–4 times), and `sequence_a_repeated` (a flat `a·a·...·a`
chain). This measures boolean membership only; no parse tree is involved.
All four groups run at Criterion's own default sample count (100) unless
`BENCH_SAMPLE_SIZE` overrides it (see above).

`BENCH_SIZE_LIMIT` (positive integer, unset = unlimited) caps every
family's hardcoded size/depth sweep to values at or below it, so a run can
be narrowed without recompiling. A family with no value at or below the
limit contributes nothing for that run, rather than erroring.


### `bench_parse.rs`

    cargo bench --bench bench_parse
    BENCH_SIZE_LIMIT=200 cargo bench --bench bench_parse
    BENCH_SAMPLE_SIZE=50 cargo bench --bench bench_parse

Benchmarks all five parsers (`deriv_std_rec`, `deriv_std_loop`, `deriv_bc`,
`pderiv_std`, `pderiv_bc`) on four families:

- `small_patterns`: fixed per-call overhead -- four fixed cases, not
  affected by `BENCH_SIZE_LIMIT`.
- `scaling_a_star`: cost as a function of input length.
- `deep_expression`: cost as a function of expression size.
- `ambiguous_star_a_or_aa`: genuine ambiguity from `(a+aa)*`.

`BENCH_SIZE_LIMIT` works the same way as in `bench_match.rs`, for the
three sweepable families. All four groups run at Criterion's own default
sample count (100) unless `BENCH_SAMPLE_SIZE` overrides it (see above).


---



## Selecting a source, match-intent, sample size, or statistical sample count

`bench_dataset` and `bench_memory` support the same three environment
variables:

| Variable | Values | Default |
|---|---|---|
| `BENCH_SOURCE` | comma-separated `suricata` (or `snort`), `spamassassin`, `regexlib` | Suricata + SpamAssassin |
| `BENCH_PATTERN_LIMIT` | positive integer | `30` |
| `BENCH_MATCH_INTENT` | `direct`, `explicit`, or `both` | `both` |

These allow runs to be narrowed without recompiling. For example:

    BENCH_SOURCE=spamassassin BENCH_PATTERN_LIMIT=10 \
      cargo bench --bench bench_dataset

    BENCH_MATCH_INTENT=explicit cargo bench --bench bench_memory

Unset `BENCH_SOURCE` is not "all sources": it means
`load_prepared_cases`'s own default (Suricata + SpamAssassin). RegexLib is
opt-in only; pass it explicitly.

`BENCH_MATCH_INTENT` filters `prepared.jsonl`'s rows by `MatchIntent`
before anything else runs: `direct` keeps only `UnanchoredAmbiguous`
rows (no anchor in the source pattern), `explicit` keeps only
`ExplicitFullMatch` rows (the source pattern's own top-level `^...$`,
stripped before translation), `both` (the default) keeps everything.
Both are generated for by `dataset_prepare`, so both are present in
`prepared.jsonl` to begin with -- this only controls which of them a given
bench run measures.

If filtering loads nothing, the benchmark panics with a per-source diagnosis
and the active filters (including the active `BENCH_MATCH_INTENT`) rather
than silently running an empty benchmark.

All three Criterion targets (`bench_match`, `bench_parse`, `bench_dataset`
-- not `bench_memory`, which isn't one) also read `BENCH_SAMPLE_SIZE`
(positive integer, `>= 10`, Criterion's own floor -- anything lower fails
with a clear message instead of Criterion's own cryptic internal assert).
Unset keeps each group's own hardcoded default; set, it replaces every
group's sample count in that bench binary. This is deliberately opt-in
rather than a new lower hardcoded default: raising it trades runtime for
statistical power, and lowering it below what's hardcoded trades away
precision, so a reader has to ask for either explicitly rather than get
it by accident.


## `bench_dataset.rs`

    cargo run --release --example dataset_prepare -- <source> <file>...
    cargo bench --bench bench_dataset
    BENCH_SAMPLE_SIZE=30 cargo bench --bench bench_dataset

Two Criterion groups:

- `dataset_<source>` (`dataset_suricata`, `dataset_spamassassin`,
  `dataset_regexlib`): all five parsers over that source's whole loaded
  corpus, one combined throughput measurement per parser.
  Admission is capped at `BENCH_PATTERN_LIMIT` distinct patterns per
  source, each contributing all of its verified variants (up to
  `--variants`, default 5, from `dataset_prepare`).
- `dataset_per_pattern`: every loaded source combined, each parser timed
  on each individual (pattern, input) entry separately.
  Uses a hardcoded limit of 5 patterns per source; `BENCH_PATTERN_LIMIT`
  does not affect this group.

An empty result produces a per-source diagnostic instead of silently measuring
an empty corpus.

Both groups lower Criterion's sample count from its own default of 100:
`dataset_<source>` to 20, `dataset_per_pattern` to 10. Each sample in
either group is one full pass over a real-corpus workload rather than a
single cheap call, so 100 samples at `deriv_std_rec`'s speed would put a
single (parser, source) combination at several minutes. `BENCH_SAMPLE_SIZE`
(see above) overrides both of these hardcoded defaults if set.


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

The three partial-derivative parsers (`deriv_bc`, `pderiv_bc`,
`pderiv_std`) allocate noticeably fewer total bytes than the two standard
derivative parsers (`deriv_std_rec`, `deriv_std_loop`). The reason is
structural: `deriv_std` retains its derivation sequence, whereas
`deriv_bc` reassigns one `ARegex` in place.


```bash

# Only spamassassin / only snort / both
BENCH_SOURCE=spamassassin cargo bench --bench bench_dataset
BENCH_SOURCE=snort        cargo bench --bench bench_dataset
BENCH_SOURCE=snort,spamassassin cargo bench --bench bench_dataset 

# BENCH_PATTERN_LIMIT at sizes worth comparing
BENCH_PATTERN_LIMIT=5   cargo bench --bench bench_dataset   # smoke-sized, fast
BENCH_PATTERN_LIMIT=15  cargo bench --bench bench_dataset
BENCH_PATTERN_LIMIT=30  cargo bench --bench bench_dataset   
BENCH_PATTERN_LIMIT=50  cargo bench --bench bench_dataset   
BENCH_PATTERN_LIMIT=150 cargo bench --bench bench_dataset  

# bench_memory configs
cargo bench --bench bench_memory                                        
BENCH_SOURCE=spamassassin BENCH_PATTERN_LIMIT=5  cargo bench --bench bench_memory
BENCH_SOURCE=snort        BENCH_PATTERN_LIMIT=5  cargo bench --bench bench_memory
BENCH_SOURCE=snort,spamassassin BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_memory
```