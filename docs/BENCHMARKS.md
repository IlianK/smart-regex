# Benchmarks

Four `cargo bench` targets, along two axes: synthetic vs. real-corpus
input, and timing vs. memory measurement.

| Target | Input | Measures | Criterion? |
|---|---|---|---|
| `bench_match` | synthetic | time | yes |
| `bench_parse` | synthetic | time | yes |
| `bench_corpus_time` | corpus | time | no |
| `bench_memory` | corpus | memory | no |

The corpus targets read `prepared.jsonl` under `data/processed/`; see
[DATASET.md](DATASET.md) for how to build that. On the two Criterion
targets, append `-- --test` to run each benchmark function once as a
quick correctness check.

## Why Criterion is not used for the corpus

Criterion's statistical model assumes a cheap closure measured many
times: 100 runs of a 10 µs call is a millisecond, and the reported CI is
meaningful. A corpus pass takes seconds to hours per parser, so measuring
it 100 times would multiply the runtime by the sample count for no
statistical gain. By the time 20 samples of an hour-long pass finish,
thermal drift, background load, and OS scheduling dominate the sample
spread that Criterion is trying to characterize.

`bench_corpus_time` and `bench_memory` are therefore plain reporting
binaries (`harness = false`). Each runs every parser once over the whole
corpus and prints the result.

## Configuration

Every target reads its configuration from the environment. No flags, no
CLI arguments.

| Variable | Applies to | Values | Default |
|---|---|---|---|
| `BENCH_SOURCE` | corpus | comma-separated `suricata`/`snort`, `spamassassin`, `regexlib`, `s2-elab`, `innovatorved` | Suricata + SpamAssassin |
| `BENCH_PATTERN_LIMIT` | corpus | positive integer | unset (no cap) |
| `BENCH_MATCH_INTENT` | corpus | `direct`, `explicit`, `both` | `both` |
| `BENCH_SPLIT_OUTCOME` | `bench_corpus_time` | `1`/`0`, `true`/`false`, `yes`/`no` | `0` |
| `BENCH_SIZE_LIMIT` | synthetic | positive integer | unset (no cap) |
| `BENCH_SAMPLE_SIZE` | Criterion targets | integer ≥ 10 | group-specific |

**`BENCH_SOURCE`.** Unset is not "all five sources": it means
`load_prepared_cases`'s own default of Suricata + SpamAssassin. RegexLib,
S2ELab, and Innovatorved are opt-in; pass them explicitly.

**`BENCH_PATTERN_LIMIT=N`.** Admits the first N distinct patterns per
source. Each admitted pattern brings all of its verified inputs, so the
bench's row count is at most `N × inputs_per_pattern`. Capping on
patterns rather than rows keeps the bench's size predictable regardless
of how many inputs each pattern carries. Unset admits every usable
pattern.

**`BENCH_MATCH_INTENT`.** Filters rows by their `MatchIntent` before
anything else runs. `direct` keeps `UnanchoredAmbiguous`; `explicit`
keeps `ExplicitFullMatch`; `both` keeps everything.

**`BENCH_SPLIT_OUTCOME`.** When set, `bench_corpus_time` prints a second
table per source splitting each parser's per-entry durations by the
entry's verified outcome (match vs. non-match). Useful when a parser's
total is skewed by one side of the corpus. Off by default; ignored by
`bench_memory`.

**`BENCH_SIZE_LIMIT=N`.** Caps each synthetic family's hardcoded
size/depth sweep to values at or below N. A family with no value at or
below the limit contributes nothing for that run.

**`BENCH_SAMPLE_SIZE=N`.** Overrides Criterion's sample count on the
bench that reads it. Unset uses each group's hardcoded default (see the
per-target sections). Opt-in so that raising precision or lowering
runtime is always explicit.

If filtering loads nothing, the corpus targets panic with a per-source
diagnosis and the active filters, rather than silently measuring an
empty corpus.

## `bench_match.rs`

```
cargo bench --bench bench_match
BENCH_SIZE_LIMIT=100    cargo bench --bench bench_match
BENCH_SAMPLE_SIZE=50    cargo bench --bench bench_match
```

Benchmarks `match_naive`, `match_deriv`, and `match_pderiv` on four
synthetic families:

- `pathological`: `(a*)*` against `aaa...a`.
- `benign`: `(a|b)*` against `abab...`.
- `nesting_depth`: `a*` nested 1–4 times.
- `sequence_a_repeated`: a flat `a·a·...·a` chain.

Measures boolean membership only; no parse tree is constructed. All
groups run at Criterion's default sample count (100) unless
`BENCH_SAMPLE_SIZE` overrides.

## `bench_parse.rs`

```
cargo bench --bench bench_parse
BENCH_SIZE_LIMIT=200    cargo bench --bench bench_parse
BENCH_SAMPLE_SIZE=50    cargo bench --bench bench_parse
```

Benchmarks all five parsers (`deriv_std_rec`, `deriv_std_loop`,
`deriv_bc`, `pderiv_std`, `pderiv_bc`) on four families:

- `small_patterns`: fixed per-call overhead, four cases, unaffected by
  `BENCH_SIZE_LIMIT`.
- `scaling_a_star`: cost as a function of input length.
- `deep_expression`: cost as a function of expression size.
- `ambiguous_star_a_or_aa`: genuine ambiguity from `(a+aa)*`.

`BENCH_SIZE_LIMIT` caps the last three. All groups run at Criterion's
default sample count (100) unless `BENCH_SAMPLE_SIZE` overrides.

## `bench_corpus_time.rs`

```
cargo bench --bench bench_corpus_time
BENCH_PATTERN_LIMIT=5     cargo bench --bench bench_corpus_time
BENCH_SOURCE=snort,regexlib cargo bench --bench bench_corpus_time
BENCH_SPLIT_OUTCOME=1     cargo bench --bench bench_corpus_time
```

One pass per source, every admitted entry timed individually. Reports,
per parser:

- **total (s)**: whole-corpus wall time.
- **mean (ms)**: per-entry average.
- **median / p99 / max (ms)**: how skewed the per-entry distribution is.

The distribution columns show whether the total is dominated by a
handful of expensive patterns, which a single aggregate figure cannot.

With `BENCH_SPLIT_OUTCOME` set, a second table splits each parser's
time by match vs. non-match, so an asymmetry in the corpus shows up as
a per-side column rather than being buried in the mean.

Reads the three corpus variables shared with `bench_memory`
(`BENCH_SOURCE`, `BENCH_PATTERN_LIMIT`, `BENCH_MATCH_INTENT`) plus its
own `BENCH_SPLIT_OUTCOME`. Not a Criterion target: one pass, no
sampling, no CI.

## `bench_memory.rs`

```
cargo bench --bench bench_memory
```

The memory counterpart to `bench_corpus_time`: reports bytes allocated
per parser per source. Not a Criterion target — allocation counts are
deterministic per input, so repeated sampling would only measure
Criterion's own overhead.

Uses a counting `#[global_allocator]` with `harness = false`. For each
source reports total allocated bytes (including freed-and-reallocated
churn), average bytes per entry, and the largest entry's peak live-byte
high-water mark.

Reads `BENCH_SOURCE`, `BENCH_PATTERN_LIMIT`, and `BENCH_MATCH_INTENT`.
`BENCH_SPLIT_OUTCOME` has no effect here.

The three partial-derivative parsers (`deriv_bc`, `pderiv_bc`,
`pderiv_std`) allocate noticeably fewer total bytes than the two
standard derivative parsers (`deriv_std_rec`, `deriv_std_loop`).
Structural reason: `deriv_std` retains its derivation sequence, whereas
`deriv_bc` reassigns one `ARegex` in place.

## Choosing a target

| Question | Target |
|---|---|
| Does the corpus read correctly? | `bench_corpus_time` with `BENCH_PATTERN_LIMIT=5` |
| How long does each parser take over the whole corpus? | `bench_corpus_time` |
| Which patterns dominate the cost? | `bench_corpus_time` (median vs. p99 vs. max) |
| Do matches and non-matches cost differently? | `bench_corpus_time` with `BENCH_SPLIT_OUTCOME=1` |
| How much memory does each parser allocate? | `bench_memory` |
| How do the parsers scale on synthetic input? | `bench_parse` |
| Does the matcher stack behave as expected on `(a*)*`? | `bench_match` |

## Quick reference

**Smoke test** — 5 patterns per source, under a minute on both corpus
targets:

```bash
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_corpus_time
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_memory
```

**Whole-corpus timing** (`bench_corpus_time`):

```bash
# Default sources (suricata, spamassassin), every pattern.
cargo bench --bench bench_corpus_time

# Capped at 50 patterns per source.
BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_corpus_time

# One source, no cap.
BENCH_SOURCE=s2-elab cargo bench --bench bench_corpus_time

# All five sources, every pattern.
BENCH_SOURCE=snort,spamassassin,regexlib,s2-elab,innovatorved \
  cargo bench --bench bench_corpus_time

# Two sources, capped at 50 patterns each.
BENCH_SOURCE=s2-elab,innovatorved \
BENCH_PATTERN_LIMIT=50 \
  cargo bench --bench bench_corpus_time

# Match vs. non-match split.
BENCH_SPLIT_OUTCOME=1 BENCH_SOURCE=s2-elab   cargo bench --bench bench_corpus_time
BENCH_SPLIT_OUTCOME=1 BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_corpus_time

# Only the patterns that carry a recoverable ^...$ wrap.
BENCH_MATCH_INTENT=explicit cargo bench --bench bench_corpus_time

# Only the unanchored patterns.
BENCH_MATCH_INTENT=direct cargo bench --bench bench_corpus_time
```

**Whole-corpus memory** (`bench_memory`, same source and pattern-limit
variables):

```bash
# Default sources, every pattern.
cargo bench --bench bench_memory

# Capped at 50 patterns per source.
BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_memory

# One source, no cap.
BENCH_SOURCE=s2-elab cargo bench --bench bench_memory

# Two sources, 30 patterns each.
BENCH_SOURCE=s2-elab,innovatorved \
BENCH_PATTERN_LIMIT=30 \
  cargo bench --bench bench_memory

# All five sources, every pattern.
BENCH_SOURCE=snort,spamassassin,regexlib,s2-elab,innovatorved \
  cargo bench --bench bench_memory

# Only unanchored patterns.
BENCH_MATCH_INTENT=direct cargo bench --bench bench_memory
```