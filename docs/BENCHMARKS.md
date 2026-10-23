# Benchmarks

Seven `cargo bench` targets, along two axes: synthetic vs. real-corpus
vs. curated-stress-pattern input, and what's measured.

| Target | Input | Measures | Criterion? |
|---|---|---|---|
| `bench_match` | synthetic | time | yes |
| `bench_parse` | synthetic | time | yes |
| `bench_corpus_time` | corpus (S2ELab) | time | no |
| `bench_memory` | corpus (S2ELab) | memory | no |
| `bench_translate` | corpus | time + memory, translation only | no |
| `bench_pathological_time` | curated stress patterns | time | no |
| `bench_pathological_memory` | curated stress patterns | memory | no |

The corpus and translation targets read `prepared.jsonl` under
`data/processed/`; see [DATASET.md](DATASET.md) for how to build that.
The two `bench_pathological_*` targets instead read
`data/pathological/pathological_patterns.json` directly -- no
preparation step, no `prepared.jsonl`. On the two Criterion targets,
append `-- --test` to run each benchmark function once as a quick
correctness check.

`bench_corpus_time`, `bench_memory`, `bench_pathological_time`, and
`bench_pathological_memory` all run every entry through one shared
worker binary, `bench_worker`, which must be built first:

```bash
cargo build --release --example bench_worker
```

Same reasoning as `demo_parser_agreement`'s own worker (see
[EXAMPLES.md](EXAMPLES.md)): isolating each call in its own process is
what lets a pathological entry be killed outright, recovering its
memory, rather than merely given up on. See "Why Criterion is not used
for the corpus" and `BENCH_ENTRY_TIMEOUT_MS` below for why that
distinction matters here specifically. The two `bench_pathological_*`
targets reuse this worker unchanged -- it already takes a generic
`{pattern, input}` job and a `parser_idx`, exactly the shape they need.

`bench_translate` does not use `bench_worker` and needs no build step
beyond itself -- it measures translation (`core_regex`) only, which
`core_regex` itself already bounds via a depth/node cap before the
expensive part runs, so there is no runaway call for process isolation
to contain. See its own section below.



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
| `BENCH_SOURCE` | `bench_translate` only | comma-separated `suricata`/`snort`, `spamassassin`, `regexlib`, `s2-elab`, `innovatorved` | S2ELab |
| `BENCH_PATTERN_LIMIT` | corpus, pathological (caps rows there) | positive integer | unset (no cap) |
| `BENCH_MATCH_INTENT` | corpus | `direct`, `explicit`, `both` | `explicit` |
| `BENCH_SPLIT_OUTCOME` | `bench_corpus_time` | `1`/`0`, `true`/`false`, `yes`/`no` | `0` |
| `BENCH_ENTRY_TIMEOUT_MS` | corpus, pathological | positive integer (milliseconds) | `2000` |
| `BENCH_RUN_LABEL` | corpus, pathological | any filename-safe string | unset (bare filename) |
| `BENCH_SIZE_LIMIT` | synthetic | positive integer | unset (no cap) |
| `BENCH_SAMPLE_SIZE` | Criterion targets | integer ≥ 10 | group-specific |
| `BENCH_TRANSLATE_RENDER_MAX_NODES` | `bench_translate` | positive integer (nodes) | `150` |
| `BENCH_TRANSLATE_EXAMPLES` | `bench_translate` | non-negative integer | `5` |
| `PATHO_JSON_PATH` | pathological | file path | `data/pathological/pathological_patterns.json` |
| `PATHO_SECTION` | pathological | `pathological`, `baseline`, `generated`, `all` | `all` |
| `PATHO_CATEGORY` | pathological | comma-separated `A`-`L` | unset (every category present) |
| `PATHO_N` | pathological | comma-separated positive integers | unset (each row's own `suggested_n`) |
| `PATHO_P` | pathological | comma-separated positive integers | unset (each row's own `suggested_p`) |


**Variables Explanation**:
- **`BENCH_SOURCE`** — `bench_translate` only; `bench_corpus_time` and
  `bench_memory` hardcode S2ELab, the only source with real prepared
  data today, and have no source selection at all. Unset means S2ELab
  only, not all five. Suricata, SpamAssassin, RegexLib, and
  Innovatorved are opt-in, though none of them has a given-input
  extraction wired in yet, so running `bench_translate` against one
  today accepts patterns and then finds every single one unusable for
  lack of example input (see `docs/DATASET.md`).

- **`BENCH_PATTERN_LIMIT=N`** — Admits the first N distinct patterns per
  source. Each brings all its given inputs, so row count is at most
  `N × inputs_per_pattern`. Unset admits every usable pattern.

- **`BENCH_MATCH_INTENT`** — Filters by `MatchIntent`. Unset means
  `explicit` — `ExplicitFullMatch` only, since an unanchored pattern's
  intended reading (search vs. full match) is genuinely ambiguous and
  benchmarking it by default would overstate how settled that reading
  is. `direct` keeps `UnanchoredAmbiguous` instead; `both` keeps
  everything.

- **`BENCH_SPLIT_OUTCOME`** — When set, `bench_corpus_time` prints a
  second table per source splitting per-entry durations by expected
  match/non-match outcome. Off by default; ignored by `bench_memory`.

- **`BENCH_ENTRY_TIMEOUT_MS=N`** — Per-entry wait cap before counting an
  entry as `timed_out`. Exists because a small fraction of real entries
  (Innovatorved, RegexLib) make `deriv_std_rec`/`deriv_std_loop` run for
  seconds to minutes; without it, one such entry blocks the whole run.
  The two `bench_pathological_*` targets read the same variable, for
  the same reason -- a pathological pattern swept to a large `n`/`p` is
  exactly the kind of call this exists to bound.
  - Each entry runs in its own `bench_worker` process, killed outright
    on timeout. A killed process reclaims memory; a thread
    stopped-waiting-on does not. The thread-based version this replaced
    OOM-killed a run; `bench_corpus_time.rs`'s module doc has the full
    account.
  - Two costs: killing is blocking, so a corpus with many slow entries
    pays ~`N_timed_out × cap` in wall clock; and each worker re-runs
    `core_regex` on the pattern, excluded from the reported per-entry
    timing.

- **`BENCH_RUN_LABEL`** — Suffixes the CSV filename
  (`bench_corpus_time_T3.csv` for `BENCH_RUN_LABEL=T3`), so a sweep of
  differently-configured runs (e.g. a `BENCH_PLAN.md`-style set of
  labelled commands) lands in separate files instead of each one
  overwriting the last. Unset keeps today's bare filename, replaced on
  every run as before. One exception: `bench_translate`'s aggregate
  `bench_translate.csv` ignores this variable entirely and always
  appends -- see its own section below.

- **`BENCH_SIZE_LIMIT=N`** — Caps each synthetic family's hardcoded
  size/depth sweep to values ≤ N. Families with no value ≤ N contribute
  nothing.

- **`BENCH_SAMPLE_SIZE=N`** — Overrides Criterion's sample count on the
  bench that reads it. Unset uses the group's hardcoded default.

- **`BENCH_TRANSLATE_RENDER_MAX_NODES=N`** — `bench_translate` only.
  Its per-pattern catalogue CSV renders a pattern's translated `Regex`
  (its derived `Debug`, showing the `Seq`/`Alt`/`Star`/`Lit`/`Eps`/`Phi`
  constructors) only when its node count is at most N; larger patterns
  get a `<N nodes, not rendered>` placeholder instead, so one oversized
  real-corpus pattern can't blow up the CSV.

- **`BENCH_TRANSLATE_EXAMPLES=N`** — `bench_translate` only. How many
  of the smallest distinct patterns per source to write, fully
  rendered, to `reports/demos/translation_examples.txt`. `0` skips
  that file entirely.

- **`PATHO_JSON_PATH`** — Both `bench_pathological_*` targets only.
  Path to the dataset file to load, in place of the default.

- **`PATHO_SECTION`** — Both `bench_pathological_*` targets only.
  Restricts rows to one of the dataset's three arrays
  (`pathological_patterns`, `baseline_and_edge_patterns`,
  `generated_patterns`); `all` (the default) reads every one.

- **`PATHO_CATEGORY`** — Both `bench_pathological_*` targets only.
  Restricts rows to the given category letters. Unset keeps every
  category present in the selected section(s).

- **`PATHO_N`** / **`PATHO_P`** — Both `bench_pathological_*` targets
  only. Overrides the swept input-length (`n`) / pattern-size (`p`)
  values for every selected row. Unset uses each row's own category's
  `suggested_n`/`suggested_p` from the dataset's own metadata, falling
  back to `pathological::DEFAULT_N`/`DEFAULT_P` (20/100) for a category
  that defines neither. `PATHO_P` only matters for a `generated_patterns`
  row with a `generator` -- every other row ignores it.

If filtering loads nothing, these targets panic with a diagnosis of
the active filters (per-source, for `bench_translate`; per
section/category, for the `bench_pathological_*` pair).


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

Build `bench_worker` first (see above), then:

```
cargo bench --bench bench_corpus_time
BENCH_PATTERN_LIMIT=5     cargo bench --bench bench_corpus_time
BENCH_SPLIT_OUTCOME=1     cargo bench --bench bench_corpus_time
```

S2ELab only, no source selection. One pass, every admitted entry timed
individually in its own `bench_worker` process, bounded by
`BENCH_ENTRY_TIMEOUT_MS`. Writes its latest run to
`reports/benches/bench_corpus_time.csv` (see below). Reports, per parser:

- **measured**: entries that neither timed out nor crashed -- the
  count the distribution columns are actually computed over. Not the
  same as the entry count printed in the section header; the gap is
  `timed_out + crashed`.
- **timed_out**: entries that exceeded the cap and were killed,
  excluded from every column below rather than folded in as if they
  were real measurements.
- **crashed**: entries whose worker exited non-zero or died by signal
  (e.g. a stack overflow) before the cap -- a different failure from a
  timeout, also excluded from the columns below.
- **wall (s)**: this process's own wall-clock time around the whole
  per-parser loop -- *not* the sum of the distribution columns below.
  It includes one `bench_worker` process spawn and one pipe round
  trip per entry, which `mean`/`median`/`p99`/`max` do not (those come
  from each worker's self-reported time around the parser call only).
  On sources of individually cheap patterns, spawn overhead can
  dominate `wall` while barely moving `mean` at all, so `wall ÷
  entries` and `mean` can differ by an order of magnitude or more and
  both still be correct -- they are not measuring the same thing.
- **mean (ms)**: per-entry average, over `measured` entries.
- **median / p99 / max (ms)**: how skewed the per-entry distribution is,
  over `measured` entries. `n/a` if every entry for that parser timed
  out or crashed. Because the entries most likely to time out are the
  ones process isolation exists to contain, `max` is a lower bound on
  the corpus's true worst case, not the true worst case itself.

The distribution columns show whether wall time is dominated by a
handful of expensive patterns, which a single aggregate figure cannot.

With `BENCH_SPLIT_OUTCOME` set, a second table splits each parser's
time by match vs. non-match, so an asymmetry in the corpus shows up as
a per-side column rather than being buried in the mean.

Reads the corpus variables shared with `bench_memory`
(`BENCH_PATTERN_LIMIT`, `BENCH_MATCH_INTENT`, `BENCH_ENTRY_TIMEOUT_MS`)
plus its own `BENCH_SPLIT_OUTCOME`. Not a Criterion target: one pass,
no sampling, no CI.

Also writes `reports/benches/bench_corpus_time.csv` (and
`bench_corpus_time_by_outcome.csv` when `BENCH_SPLIT_OUTCOME=1`),
replacing any file from a previous run -- the same "latest run, not a
log" convention `dataset_prepare` uses for `prepared.jsonl`. This is
what a plain binary needs that Criterion gave for free: something
persisted once the terminal scrolls away. Set `BENCH_RUN_LABEL` to
give each configuration its own file instead of overwriting the last.




## `bench_memory.rs`

Build `bench_worker` first (see above), then:

```
cargo bench --bench bench_memory
```

S2ELab only, no source selection, the same as `bench_corpus_time`. The
heap-allocation counterpart to `bench_corpus_time`: reports bytes
allocated per parser. "Heap" specifically, not memory in
general -- the counting allocator sees `GlobalAlloc` traffic only, so
a cost dominated by stack depth rather than allocation would not show
up here. Not a Criterion target -- allocation counts are deterministic
per input, so repeated sampling would only measure Criterion's own
overhead.

The counting `#[global_allocator]` lives in `bench_worker`, not in
this binary -- each entry's worker process measures its own
allocations from a clean baseline and reports them back, so this file
itself does no allocation counting. Writes its latest run to
`reports/benches/bench_memory.csv` (replacing any file from a
previous run; `BENCH_RUN_LABEL` gives it its own file instead, same as
`bench_corpus_time`). For each source reports, per parser:

- **measured**: entries that neither timed out nor crashed -- the
  count `avg_bytes`/`max peak` are actually computed over.
- **timed_out** / **crashed**: see `BENCH_ENTRY_TIMEOUT_MS` above for
  both. A `timed_out` entry's worker is killed before it ever writes a
  result line, so it contributes nothing to `max peak` below -- not a
  value capped at the timeout cap, a full exclusion. The entries most
  likely to time out are exactly the ones process isolation exists to
  contain, so `max peak` is a lower bound on the corpus's true worst
  case, not the true worst case itself.
- **total bytes**: every allocation during a call, including freed
  -and-reallocated churn, over `measured` entries.
- **avg/entry**: `total bytes` divided by `measured`.
- **max peak**: the highest live-byte high-water mark seen on any
  single `measured` entry.

The three partial-derivative parsers (`deriv_bc`, `pderiv_bc`,
`pderiv_std`) allocate noticeably fewer total bytes than the two
standard derivative parsers (`deriv_std_rec`, `deriv_std_loop`).
Structural reason: `deriv_std` retains its derivation sequence, whereas
`deriv_bc` reassigns one `ARegex` in place.

## `bench_translate.rs`

No worker to build first -- see "Five `cargo bench` targets" above for
why not. Then:

```
cargo bench --bench bench_translate
BENCH_SOURCE=suricata cargo bench --bench bench_translate
```

Cost of frontend translation (`core_regex`: PCRE string -> `Regex`)
only -- the step every other target here excludes (see
`bench_worker.rs`'s module doc). Measured once per *distinct pattern*,
not per entry: translation cost depends only on the pattern, and an
entry-level measurement would re-measure the same pattern's
translation once per input it carries.

Three outputs, all under `reports/`. Two are labelled like every other
bench's output here; the aggregate one is the one exception in this
whole file:

- `reports/benches/bench_translate.csv` -- one row per source:
  mean/median/p99/max of time (ms), allocation (total and peak bytes),
  and translated size (nodes), over every pattern that translated
  successfully (`translated`; `rejected` and `crashed` are excluded
  the same way `timed_out`/`crashed` are excluded elsewhere in this
  file, and for the same reason -- they are not real measurements of
  a successful call). Always exactly this filename, ignoring
  `BENCH_RUN_LABEL` -- **appended** to, never overwritten, so a run
  scoped to one source and a run covering all five both add their
  row(s) to the same growing file instead of replacing each other.
  Two extra columns, `run_label` and `pattern_limit`, record each
  row's own invocation, since the (shared) filename no longer can.
- `reports/benches/bench_translate{label}_patterns.csv` -- one row per
  distinct pattern: its own nodes/micros/bytes, plus the translated
  `Regex` itself via its derived `Debug` (`Seq(Lit('a'), ...)`-style --
  the six constructors translation actually produces, not a flattened
  reading of them) when `BENCH_TRANSLATE_RENDER_MAX_NODES` allows it.
  The catalogue to hand-pick thesis examples from. Labelled and
  overwritten as usual -- kept one catalogue per run, on purpose, even
  though the aggregate CSV next to it isn't.
- `reports/demos/translation_examples.txt` -- the
  `BENCH_TRANSLATE_EXAMPLES` smallest distinct patterns per source,
  fully rendered. Always overwritten, never labelled by
  `BENCH_RUN_LABEL`: a curated starting point, not a measurement run.

Reads `BENCH_SOURCE`, `BENCH_PATTERN_LIMIT`, and `BENCH_MATCH_INTENT`
the same way `bench_corpus_time`/`bench_memory` do.
`BENCH_RUN_LABEL` only affects the `_patterns.csv` filename -- the
aggregate CSV ignores it (see above), and the examples file is never
labelled either way. Does not read `BENCH_ENTRY_TIMEOUT_MS` or
`BENCH_SPLIT_OUTCOME`: there is no timeout concept here (see "Five
`cargo bench` targets" above) and no match/non-match split
(translation doesn't touch the input at all).

No process isolation, unlike every other corpus target in this file.
Deliberate, not an oversight: every pattern reaching this bench already
passed `core_regex` once during `dataset_prepare`, and `core_regex`
enforces its own depth/node cap before the expensive part of
translation runs -- a pattern that could not blow up there cannot blow
up here either, same function, same input, same cap. A per-pattern
`catch_unwind` stands in for process isolation's crash containment, so
one unexpected panic costs one row (counted as `crashed`), not the
whole run. One consequence of skipping isolation: this bench's timing
has no process-spawn overhead, unlike `wall (s)` in
`bench_corpus_time`.

## `bench_pathological_time.rs`

Build `bench_worker` first (see above), then:

```
cargo bench --bench bench_pathological_time
PATHO_CATEGORY=A,B               cargo bench --bench bench_pathological_time
PATHO_SECTION=generated PATHO_P=1000 cargo bench --bench bench_pathological_time
```

Wall-clock cost per parser over the hand-curated stress-pattern
dataset (`data/pathological/pathological_patterns.json`), not the real
corpus -- that one is `bench_corpus_time`. Each selected row's
`match_input`/`nomatch_input` is expanded at every swept `n` (and, for
a `generated_patterns` row with a `generator`, every swept `p` too),
each expansion becoming up to two entries (match + non-match: a row
whose `match_input` is `null`, like the `∅` row, contributes only the
non-match side). Entries are grouped by category (A-L) for reporting
-- the same role `source` plays in `bench_corpus_time` -- and reported
the same way: `measured`/`timed_out`/`crashed`/`wall (s)`/`mean`/
`median`/`p99`/`max (ms)`, per parser, per category.

Reuses `bench_worker` unchanged and the identical
timeout/kill/crash accounting as `bench_corpus_time` (see that
section above and `BENCH_ENTRY_TIMEOUT_MS` below). One difference: no
pre-filtering by `core_regex` before spawning a worker -- a
pathological pattern's translation itself failing (hitting
`core_regex`'s own depth/node cap, e.g. a category J pattern swept to
a very large `p`) is a real outcome worth seeing here, so it is left
to show up as `crashed` rather than being filtered out beforehand.

Reads `BENCH_PATTERN_LIMIT` (caps selected *rows*, before the `n`/`p`
sweep), `BENCH_ENTRY_TIMEOUT_MS`, `BENCH_RUN_LABEL`, and its own
`PATHO_JSON_PATH`/`PATHO_SECTION`/`PATHO_CATEGORY`/`PATHO_N`/`PATHO_P`.
No `BENCH_MATCH_INTENT` or `BENCH_SPLIT_OUTCOME`: these patterns carry
no anchor/match-intent concept, and match vs. non-match cost is not
split out separately here.

Writes `reports/benches/bench_pathological_time.csv`, replacing any
existing file, same "latest run, not a log" convention as
`bench_corpus_time`.

## `bench_pathological_memory.rs`

Build `bench_worker` first (see above), then:

```
cargo bench --bench bench_pathological_memory
```

The heap-allocation counterpart to `bench_pathological_time`, same
relationship as `bench_memory` to `bench_corpus_time`: same dataset,
same row/category grouping, same sweep, same `PATHO_*` variables, same
worker reuse and timeout/kill/crash accounting, but reporting bytes
(`total_bytes`/`avg_bytes`/`max_peak`) instead of durations. See
`bench_memory.rs`'s own section above for what "heap" does and does
not cover.

Writes `reports/benches/bench_pathological_memory.csv`, replacing any
existing file.

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
| How much does translation itself cost, separately from parsing? | `bench_translate` |
| What does a real pattern's translated form actually look like? | `bench_translate`'s `_patterns.csv` or `translation_examples.txt` |
| How does a parser scale on a known-pathological pattern, by category? | `bench_pathological_time` |
| How much does a pathological pattern allocate as it's swept larger? | `bench_pathological_memory` |




## Quick reference

**Smoke test** — 5 patterns, under a minute on both corpus targets:

```bash
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_corpus_time
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_memory
```

**Whole-corpus timing** (`bench_corpus_time`, S2ELab only, no source
selection):

```bash
# Every pattern.
cargo bench --bench bench_corpus_time

# Capped at 50 patterns.
BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_corpus_time

# Match vs. non-match split.
BENCH_SPLIT_OUTCOME=1 BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_corpus_time

# Only the patterns that carry a recoverable ^...$ wrap.
BENCH_MATCH_INTENT=explicit cargo bench --bench bench_corpus_time

# Only the unanchored patterns.
BENCH_MATCH_INTENT=direct cargo bench --bench bench_corpus_time
```

**Whole-corpus memory** (`bench_memory`, same pattern-limit variable,
also S2ELab only):

```bash
# Every pattern.
cargo bench --bench bench_memory

# Capped at 50 patterns.
BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_memory

# Only unanchored patterns.
BENCH_MATCH_INTENT=direct cargo bench --bench bench_memory
```

**Translation cost** (`bench_translate`, no worker to build, its own
source/pattern-limit variables -- the only one of these targets that
still selects a source):

One source per invocation; each one's own label keeps its
`_patterns.csv` genuinely separate per source, which is the point of
that file. Each run still appends its row to the single, shared
`bench_translate.csv` overview regardless of label, so that file ends
up covering every source without needing a combined run.

```bash
# Default sources, every distinct pattern.
cargo bench --bench bench_translate

# One source, no cap -- appends its own row to the shared overview CSV.
BENCH_SOURCE=suricata BENCH_RUN_LABEL=X2 cargo bench --bench bench_translate

# Render fewer/more examples, or render larger patterns' trees too.
BENCH_TRANSLATE_EXAMPLES=10 BENCH_TRANSLATE_RENDER_MAX_NODES=300 \
  cargo bench --bench bench_translate
```

**Pathological stress patterns** (`bench_pathological_time` /
`bench_pathological_memory`, own `PATHO_*` variables):

```bash
# Cheap smoke test: small n/p, few rows, tight timeout.
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 \
  cargo bench --bench bench_pathological_time

# Every row, every category, each row's own suggested n/p sweep.
cargo bench --bench bench_pathological_time
cargo bench --bench bench_pathological_memory

# Just the nested-quantifier and overlapping-alternation categories.
PATHO_CATEGORY=A,B cargo bench --bench bench_pathological_time

# Only the generated_patterns section, overriding its pattern-size sweep.
PATHO_SECTION=generated PATHO_P=1000 cargo bench --bench bench_pathological_memory
```