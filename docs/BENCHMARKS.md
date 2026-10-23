# Benchmarks

Six `cargo bench` targets, grouped by dataset and by what they
measure. There are exactly two datasets today: the real S2ELab corpus
(`bench_corpus_*`) and the hand-curated pathological stress-pattern
set (`bench_pathological_*`). Each has three targets, one per axis:

| Target | Dataset | Measures |
|---|---|---|
| `bench_corpus_time` | S2ELab corpus | parser wall-clock time |
| `bench_corpus_memory` | S2ELab corpus | parser heap allocation |
| `bench_corpus_translate` | S2ELab corpus | translation time + memory + node count |
| `bench_pathological_time` | pathological stress patterns | parser wall-clock time |
| `bench_pathological_memory` | pathological stress patterns | parser heap allocation |
| `bench_pathological_translate` | pathological stress patterns | translation time + memory + node count |

None of these use Criterion -- see "Why none of these targets use
Criterion" below. All six are plain reporting binaries (`harness =
false`) that print a table and also write it to a CSV under
`reports/benches/`.

The `bench_corpus_*` targets read `prepared.jsonl` under
`data/processed/`; see [DATASET.md](DATASET.md) for how to build that.
The `bench_pathological_*` targets instead read
`data/pathological/pathological_patterns.json` directly -- no
preparation step, no `prepared.jsonl`.

`bench_corpus_time`, `bench_corpus_memory`, `bench_pathological_time`,
and `bench_pathological_memory` all run every entry through one shared
worker binary, `bench_worker`, which must be built first:

```bash
cargo build --release --example bench_worker
```

Same reasoning as `demo_parser_agreement`'s own worker (see
[EXAMPLES.md](EXAMPLES.md)): isolating each call in its own process is
what lets a pathological entry be killed outright, recovering its
memory, rather than merely given up on. See "Why Criterion is not
used" and `BENCH_ENTRY_TIMEOUT_MS` below for why that distinction
matters here specifically. `bench_pathological_time`/
`bench_pathological_memory` reuse this worker unchanged -- it already
takes a generic `{pattern, input}` job and a `parser_idx`, exactly the
shape they need.

Neither translate-only target (`bench_corpus_translate`,
`bench_pathological_translate`) uses `bench_worker`, and neither needs
a build step beyond itself -- both measure translation (`core_regex`)
only, which `core_regex` itself already bounds via a depth/node cap
before the expensive part runs, so there is no runaway call for
process isolation to contain. See their own sections below.



## Why none of these targets use Criterion

Criterion's statistical model assumes a cheap closure measured many
times: 100 runs of a 10 µs call is a millisecond, and the reported CI is
meaningful. A corpus pass takes seconds to hours per parser, so measuring
it 100 times would multiply the runtime by the sample count for no
statistical gain. By the time 20 samples of an hour-long pass finish,
thermal drift, background load, and OS scheduling dominate the sample
spread that Criterion is trying to characterize. The same argument
applies to a pathological-dataset pass swept across several `n`/`p`
values: cheaper per call on average, but with the same long tail that
process isolation exists to bound in the first place.

This project used to also have two Criterion-based targets,
`bench_match` and `bench_parse`, which benchmarked hand-built
synthetic `Regex` ASTs (`Regex::star(Regex::lit('a'))`-style helpers)
rather than real pattern strings -- bypassing `parse_pattern`/
`translate`, the real frontend, entirely. `bench_match` measured the
separate `matchers` module (`match_naive`/`match_deriv`/
`match_pderiv`), already out of scope project-wide; `bench_parse`
measured the right subject (the five real parsers) but its synthetic
shapes were a strict subset of what the pathological dataset's `A`/
`H`/`L` categories now cover through the real string-parsing pipeline,
which is more representative. Both were removed, and `criterion` with
them -- it had no other consumer.

So every target in this file, whether it runs once over the whole
S2ELab corpus or sweeps the pathological dataset across several
`n`/`p` values, is a plain reporting binary. Each runs every parser
(or, for the `*_translate` pair, just `core_regex`) once per selected
entry and prints the result.



## Configuration

Every target reads its configuration from the environment. No flags, no
CLI arguments.

| Variable | Applies to | Values | Default |
|---|---|---|---|
| `BENCH_SOURCE` | `bench_corpus_translate` only | comma-separated `suricata`/`snort`, `spamassassin`, `regexlib`, `s2-elab`, `innovatorved` | S2ELab |
| `BENCH_PATTERN_LIMIT` | all six (caps selected *rows* for the pathological trio, before their `n`/`p` sweep) | positive integer | unset (no cap) |
| `BENCH_MATCH_INTENT` | `bench_corpus_time`, `bench_corpus_memory`, `bench_corpus_translate` | `direct`, `explicit`, `both` | `explicit` |
| `BENCH_SPLIT_OUTCOME` | `bench_corpus_time` only | `1`/`0`, `true`/`false`, `yes`/`no` | `0` |
| `BENCH_ENTRY_TIMEOUT_MS` | `bench_corpus_time`, `bench_corpus_memory`, `bench_pathological_time`, `bench_pathological_memory` (not either `*_translate` target -- no process isolation to bound) | positive integer (milliseconds) | `2000` |
| `BENCH_RUN_LABEL` | all six, with caveats -- see below | any filename-safe string | unset (bare filename) |
| `BENCH_TRANSLATE_RENDER_MAX_NODES` | `bench_corpus_translate` only | positive integer (nodes) | `150` |
| `BENCH_TRANSLATE_EXAMPLES` | `bench_corpus_translate` only | non-negative integer | `5` |
| `PATHO_JSON_PATH` | all three `bench_pathological_*` targets | file path | `data/pathological/pathological_patterns.json` |
| `PATHO_SECTION` | all three `bench_pathological_*` targets | `pathological`, `baseline`, `generated`, `all` | `all` |
| `PATHO_CATEGORY` | all three `bench_pathological_*` targets | comma-separated `A`-`L` | unset (every category present) |
| `PATHO_N` | `bench_pathological_time`, `bench_pathological_memory` only -- **not** `bench_pathological_translate`, see its own section | comma-separated positive integers | unset (each row's own `suggested_n`) |
| `PATHO_P` | all three `bench_pathological_*` targets | comma-separated positive integers | unset (each row's own `suggested_p`) |


**Variables Explanation**:
- **`BENCH_SOURCE`** — `bench_corpus_translate` only;
  `bench_corpus_time` and `bench_corpus_memory` hardcode S2ELab, the
  only source with real prepared data today, and have no source
  selection at all. Unset means S2ELab only, not all five. Suricata,
  SpamAssassin, RegexLib, and Innovatorved are opt-in, though none of
  them has a given-input extraction wired in yet, so running
  `bench_corpus_translate` against one today accepts patterns and then
  finds every single one unusable for lack of example input (see
  `docs/DATASET.md`).

- **`BENCH_PATTERN_LIMIT=N`** — For the two corpus-time/memory targets
  and `bench_corpus_translate`: admits the first N distinct patterns
  per source. Each brings all its given inputs, so row count is at
  most `N × inputs_per_pattern`. For the `bench_pathological_*` trio:
  caps the number of *rows* admitted, after section/category
  filtering and before the `n`/`p` sweep -- a row still contributes up
  to two entries (match + non-match) once expanded, or several pattern
  measurements for `bench_pathological_translate` if it sweeps `p`.
  Unset admits everything selected.

- **`BENCH_MATCH_INTENT`** — Filters by `MatchIntent`. Unset means
  `explicit` — `ExplicitFullMatch` only, since an unanchored pattern's
  intended reading (search vs. full match) is genuinely ambiguous and
  benchmarking it by default would overstate how settled that reading
  is. `direct` keeps `UnanchoredAmbiguous` instead; `both` keeps
  everything. The pathological dataset carries no such concept, so
  none of the `bench_pathological_*` targets read this variable.

- **`BENCH_SPLIT_OUTCOME`** — When set, `bench_corpus_time` prints a
  second table per source splitting per-entry durations by expected
  match/non-match outcome. Off by default; no other target reads it.

- **`BENCH_ENTRY_TIMEOUT_MS=N`** — Per-entry wait cap before counting an
  entry as `timed_out`. Exists because a small fraction of real entries
  (Innovatorved, RegexLib) make `deriv_std_rec`/`deriv_std_loop` run for
  seconds to minutes; without it, one such entry blocks the whole run.
  `bench_pathological_time`/`bench_pathological_memory` read the same
  variable, for the same reason -- a pathological pattern swept to a
  large `n`/`p` is exactly the kind of call this exists to bound.
  Neither `*_translate` target reads it: `core_regex` bounds itself
  before the expensive part of translation runs, so there is no
  runaway call here to cap.
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
  differently-configured runs lands in separate files instead of each
  one overwriting the last. Unset keeps today's bare filename, replaced
  on every run as before. This is how `bench_corpus_time`,
  `bench_corpus_memory`, `bench_pathological_time`, and
  `bench_pathological_memory` all behave. The two `*_translate`
  targets' own aggregate CSVs (`bench_corpus_translate.csv`,
  `bench_pathological_translate.csv`) are the exception: both ignore
  this variable for their *filename* -- always exactly that name,
  appended to, never overwritten -- and instead record it as a
  `run_label` column on each row, since the shared filename no longer
  can. `bench_corpus_translate`'s secondary `_patterns.csv` catalogue
  *does* still use it for the filename, same as the four plain targets.

- **`BENCH_TRANSLATE_RENDER_MAX_NODES=N`** — `bench_corpus_translate`
  only. Its per-pattern catalogue CSV renders a pattern's translated
  `Regex` (its derived `Debug`, showing the `Seq`/`Alt`/`Star`/`Lit`/
  `Eps`/`Phi` constructors) only when its node count is at most N;
  larger patterns get a `<N nodes, not rendered>` placeholder instead,
  so one oversized real-corpus pattern can't blow up the CSV.
  `bench_pathological_translate` has no equivalent catalogue or
  rendering step -- see its own section for why.

- **`BENCH_TRANSLATE_EXAMPLES=N`** — `bench_corpus_translate` only. How
  many of the smallest distinct patterns per source to write, fully
  rendered, to `reports/demos/translation_examples.txt`. `0` skips
  that file entirely.

- **`PATHO_JSON_PATH`** — All three `bench_pathological_*` targets.
  Path to the dataset file to load, in place of the default.

- **`PATHO_SECTION`** — All three `bench_pathological_*` targets.
  Restricts rows to one of the dataset's three arrays
  (`pathological_patterns`, `baseline_and_edge_patterns`,
  `generated_patterns`); `all` (the default) reads every one.

- **`PATHO_CATEGORY`** — All three `bench_pathological_*` targets.
  Restricts rows to the given category letters. Unset keeps every
  category present in the selected section(s).

- **`PATHO_N`** — `bench_pathological_time`/`bench_pathological_memory`
  only. Overrides the swept input-length (`n`) values for every
  selected row. Unset uses each row's own category's `suggested_n`
  from the dataset's own metadata, falling back to
  `pathological::DEFAULT_N` (20) for a category that defines none.
  **Has no effect on `bench_pathological_translate`**: a pattern's
  translated form never depends on the input it will later be matched
  against, only on the pattern text itself -- see that target's own
  section.

- **`PATHO_P`** — All three `bench_pathological_*` targets. Overrides
  the swept pattern-size (`p`) values for every selected row that
  needs one. Unset uses each row's own category's `suggested_p`,
  falling back to `pathological::DEFAULT_P` (100) for a category that
  defines none. Only matters for a `generated_patterns` row with a
  `generator` -- every other row ignores it, for all three targets
  alike, since its pattern doesn't depend on `p` either.

If filtering loads nothing, these targets panic with a diagnosis of
the active filters (per-source, for `bench_corpus_translate`; per
section/category, for the `bench_pathological_*` trio).


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

Reads the corpus variables shared with `bench_corpus_memory`
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




## `bench_corpus_memory.rs`

Build `bench_worker` first (see above), then:

```
cargo bench --bench bench_corpus_memory
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
`reports/benches/bench_corpus_memory.csv` (replacing any file from a
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

## `bench_corpus_translate.rs`

No worker to build first -- see "Six `cargo bench` targets" above for
why not. Then:

```
cargo bench --bench bench_corpus_translate
BENCH_SOURCE=suricata cargo bench --bench bench_corpus_translate
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

- `reports/benches/bench_corpus_translate.csv` -- one row per source:
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
- `reports/benches/bench_corpus_translate{label}_patterns.csv` -- one
  row per distinct pattern: its own nodes/micros/bytes, plus the
  translated `Regex` itself via its derived `Debug` (`Seq(Lit('a'),
  ...)`-style -- the six constructors translation actually produces,
  not a flattened reading of them) when `BENCH_TRANSLATE_RENDER_MAX_NODES`
  allows it. The catalogue to hand-pick thesis examples from. Labelled
  and overwritten as usual -- kept one catalogue per run, on purpose,
  even though the aggregate CSV next to it isn't.
- `reports/demos/translation_examples.txt` -- the
  `BENCH_TRANSLATE_EXAMPLES` smallest distinct patterns per source,
  fully rendered. Always overwritten, never labelled by
  `BENCH_RUN_LABEL`: a curated starting point, not a measurement run.

Reads `BENCH_SOURCE`, `BENCH_PATTERN_LIMIT`, and `BENCH_MATCH_INTENT`
the same way `bench_corpus_time`/`bench_corpus_memory` do.
`BENCH_RUN_LABEL` only affects the `_patterns.csv` filename -- the
aggregate CSV ignores it (see above), and the examples file is never
labelled either way. Does not read `BENCH_ENTRY_TIMEOUT_MS` or
`BENCH_SPLIT_OUTCOME`: there is no timeout concept here (see "Six
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
to show up as `crashed` rather than being filtered out beforehand. (Its
sibling target, `bench_pathological_translate`, is what actually
measures that translation cost directly, rather than only observing it
as an occasional `crashed` row here.)

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
relationship as `bench_corpus_memory` to `bench_corpus_time`: same
dataset, same row/category grouping, same sweep, same `PATHO_*`
variables, same worker reuse and timeout/kill/crash accounting, but
reporting bytes (`total_bytes`/`avg_bytes`/`max_peak`) instead of
durations. See `bench_corpus_memory.rs`'s own section above for what
"heap" does and does not cover.

Writes `reports/benches/bench_pathological_memory.csv`, replacing any
existing file.

## `bench_pathological_translate.rs`

No worker to build first -- same reasoning as `bench_corpus_translate`
(see "Six `cargo bench` targets" above). Then:

```
cargo bench --bench bench_pathological_translate
PATHO_CATEGORY=C,D,J,K           cargo bench --bench bench_pathological_translate
PATHO_SECTION=generated PATHO_P=100000 cargo bench --bench bench_pathological_translate
```

The translation-only counterpart to `bench_pathological_time`/
`bench_pathological_memory`, the same relationship
`bench_corpus_translate` has to `bench_corpus_time`/
`bench_corpus_memory`. Why it exists: categories **C** (bounded
repetition over a star), **D** (anchored-failure style), **J** (large
pattern, trivial input), and **K** (large pattern + large input) are
explicitly built around compile/translate blow-up -- `{m,n}`-bound
unrolling (`repeat_exact`/`optional_extra` in
`src/frontend/translate.rs`) for C/D, large alternation/group trees for
J/K -- as their own `stress_axis`. `bench_pathological_time` and
`bench_pathological_memory` both measure the parser call on an
*already-translated* `Regex` (the same baseline-after-translate design
`bench_corpus_time`/`bench_corpus_memory` use), so translation cost
itself -- the actual axis those four categories exist to stress -- was
never measured anywhere. This target closes that gap.

Row selection (`PATHO_JSON_PATH`/`PATHO_SECTION`/`PATHO_CATEGORY`/
`BENCH_PATTERN_LIMIT`) and pattern resolution reuse
`regex_engine::data::pathological::{load, expand}` directly, the same
functions `bench_pathological_time`/`bench_pathological_memory` use --
not reimplemented. One real difference from those two: this target
sweeps `PATHO_P` only, never `PATHO_N`. A row's *translated pattern*
depends only on its pattern text -- for a `generated_patterns` row
with a `generator`, that text is resolved from `pattern_template` at a
given swept `p` (pattern size); `n` (input length) only ever
parameterizes `match_input`/`nomatch_input`, which translation never
touches. So `PATHO_N` is read nowhere in this file and has no effect
here at all -- not an oversight, the same reasoning
`bench_corpus_translate` uses for not reading `BENCH_ENTRY_TIMEOUT_MS`
(nothing here for that variable to act on). A row with no `generator`
is measured exactly once, regardless of any `PATHO_P`/`PATHO_N`
setting, since neither moves its pattern; a `generated_patterns` row
with a `generator` is measured once per distinct swept `p` -- the
category-J/K blow-up curve this target exists to show.

Entries are grouped by category (A-L), same role `source` plays in
`bench_corpus_translate`'s summary -- and, like that summary, there is
no per-parser breakdown: translation doesn't depend on which parser
will run later. Reports, per category:

- **rows**: pattern entries selected for that category (rows × their
  distinct swept `p` values).
- **translated** / **rejected** / **crashed**: same three-way split as
  `bench_corpus_translate`'s own summary. `rejected` means `core_regex`
  itself returned `Err` -- e.g. hit its depth/node cap -- which, for
  categories C/D/J/K at an aggressive `PATHO_P`, is a real finding
  worth seeing directly, not something to filter out beforehand.
  `crashed` means the call panicked (caught via `catch_unwind`, one
  row's cost, not the whole run).
- **mean/median/p99/max** of time (ms), total bytes, peak bytes, and
  node count, over `translated` entries only.

Unlike `bench_corpus_translate`, this target writes only one output
file -- no per-pattern `_patterns.csv` catalogue and no
`translation_examples.txt`. The pathological dataset is 106 hand
-curated rows with their own `id`/`notes` already documented in
`pathological_patterns.json` itself; a separate catalogue to hand-pick
examples from would just restate that. `BENCH_TRANSLATE_RENDER_MAX_NODES`
and `BENCH_TRANSLATE_EXAMPLES` are therefore not read here.

Writes `reports/benches/bench_pathological_translate.csv` -- one row
per category: mean/median/p99/max of time, allocation, and node count,
over every row/`p` combination that translated successfully. Always
this exact filename, ignoring `BENCH_RUN_LABEL` for its *name* --
**appended** to, never overwritten, the same convention
`bench_corpus_translate.csv` uses for its own aggregate CSV, so a run
scoped to one category and a run covering every category both add
their row(s) to the same growing file. `run_label` and `pattern_limit`
columns record each row's own invocation, since the filename no longer
can.

## Choosing a target

| Question | Target |
|---|---|
| Does the corpus read correctly? | `bench_corpus_time` with `BENCH_PATTERN_LIMIT=5` |
| How long does each parser take over the whole corpus? | `bench_corpus_time` |
| Which patterns dominate the cost? | `bench_corpus_time` (median vs. p99 vs. max) |
| Do matches and non-matches cost differently? | `bench_corpus_time` with `BENCH_SPLIT_OUTCOME=1` |
| How much memory does each parser allocate? | `bench_corpus_memory` |
| How much does translation itself cost, separately from parsing? | `bench_corpus_translate` |
| What does a real pattern's translated form actually look like? | `bench_corpus_translate`'s `_patterns.csv` or `translation_examples.txt` |
| How does a parser scale on a known-pathological pattern, by category? | `bench_pathological_time` |
| How much does a pathological pattern allocate as it's swept larger? | `bench_pathological_memory` |
| How much does *translation itself* cost on a pattern built to stress compile-time blow-up (categories C/D/J/K)? | `bench_pathological_translate` |



## Quick reference

**Smoke test** — a handful of patterns/rows, under a minute on every
target:

```bash
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_corpus_time
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_corpus_memory
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_corpus_translate

PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 \
  cargo bench --bench bench_pathological_time
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 \
  cargo bench --bench bench_pathological_memory
PATHO_P=5 BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_pathological_translate
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

**Whole-corpus memory** (`bench_corpus_memory`, same pattern-limit
variable, also S2ELab only):

```bash
# Every pattern.
cargo bench --bench bench_corpus_memory

# Capped at 50 patterns.
BENCH_PATTERN_LIMIT=50 cargo bench --bench bench_corpus_memory

# Only unanchored patterns.
BENCH_MATCH_INTENT=direct cargo bench --bench bench_corpus_memory
```

**Translation cost** (`bench_corpus_translate`, no worker to build,
its own source/pattern-limit variables -- the only one of these six
targets that still selects a source):

One source per invocation; each one's own label keeps its
`_patterns.csv` genuinely separate per source, which is the point of
that file. Each run still appends its row to the single, shared
`bench_corpus_translate.csv` overview regardless of label, so that
file ends up covering every source without needing a combined run.

```bash
# Default sources, every distinct pattern.
cargo bench --bench bench_corpus_translate

# One source, no cap -- appends its own row to the shared overview CSV.
BENCH_SOURCE=suricata BENCH_RUN_LABEL=X2 cargo bench --bench bench_corpus_translate

# Render fewer/more examples, or render larger patterns' trees too.
BENCH_TRANSLATE_EXAMPLES=10 BENCH_TRANSLATE_RENDER_MAX_NODES=300 \
  cargo bench --bench bench_corpus_translate
```

**Pathological stress patterns** (`bench_pathological_time` /
`bench_pathological_memory` / `bench_pathological_translate`, shared
`PATHO_*` variables -- `PATHO_N` only matters to the first two, see
above):

```bash
# Every row, every category, each row's own suggested n/p sweep.
cargo bench --bench bench_pathological_time
cargo bench --bench bench_pathological_memory
cargo bench --bench bench_pathological_translate

# Just the nested-quantifier and overlapping-alternation categories.
PATHO_CATEGORY=A,B cargo bench --bench bench_pathological_time

# Only the generated_patterns section, overriding its pattern-size sweep.
PATHO_SECTION=generated PATHO_P=1000 cargo bench --bench bench_pathological_memory

# Translation cost on exactly the four compile-blow-up categories.
PATHO_CATEGORY=C,D,J,K cargo bench --bench bench_pathological_translate
```



## Bench Plan

This project used to carry these as a standalone `docs/BENCH_PLAN.md`,
written against an older, five-source, multi-axis input pipeline. That
pipeline is gone (today there are exactly two datasets, each with its
own fixed shape -- see the overview table at the top of this file), so
rather than resurrect that file verbatim, this section replaces it
with a plan sized to the current reality: two datasets × three
measurement axes = six targets, each with a cheap smoke config and a
full-sweep config.

**Naming scheme.** Each config name is `<dataset><axis><tier>`:
- First letter, dataset: `C` = the S2ELab **c**orpus, `P` = the
  **p**athological stress set.
- Second letter, axis: `T` = parser **t**ime, `M` = parser **m**emory,
  `X` = translation cost only (kept as `X` for continuity with this
  project's older `bench_translate_X2`-style run labels, still visible
  today under their original names as
  `reports/benches/bench_translate_X2_patterns.csv` and siblings --
  those files are historical data, not renamed along with the code).
- Trailing digit, tier: `0` = smoke (seconds, for "does this still
  work"), `1` = the full sweep (for a real number to report).

This gives exactly the twelve configs the three bullets below ask
for: a smoke config for each of the six targets (the `*0` row), a full
S2ELab sweep (`CT1`/`CM1`/`CX1`), and a full pathological sweep per
category (`PT1`/`PM1`/`PX1` -- "per category" is automatic here, not
an extra loop: every target in this file already groups its report by
category/source, and leaving `PATHO_CATEGORY` unset, as every `*1`
config below does, selects every category present in one run).

Every command sets its own `BENCH_RUN_LABEL` matching the config name,
so each one's CSV lands in its own file under `reports/benches/`
(`bench_corpus_time_CT1.csv`, `bench_pathological_memory_PM0.csv`,
...) instead of overwriting another config's run. The two `*_translate`
targets' aggregate CSVs are the standing exception described under
`BENCH_RUN_LABEL` above: the label still gets recorded as a column on
each appended row, it just never renames `bench_corpus_translate.csv`/
`bench_pathological_translate.csv` themselves.

All six `*0` smoke configs below were run for real while writing this
section and confirmed working end to end (correct entry/row counts,
correct CSV path, non-empty rows) -- not merely compiled.

### Corpus time (`CT0`/`CT1`)

```bash
# CT0 -- smoke: 5 patterns, ~10 entries, 1s cap.
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CT0 \
  cargo bench --bench bench_corpus_time

# CT1 -- full S2ELab sweep: every pattern, match/non-match split.
BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=CT1 \
  cargo bench --bench bench_corpus_time
```

### Corpus memory (`CM0`/`CM1`)

```bash
# CM0 -- smoke: 5 patterns, 1s cap.
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CM0 \
  cargo bench --bench bench_corpus_memory

# CM1 -- full S2ELab sweep: every pattern.
BENCH_RUN_LABEL=CM1 cargo bench --bench bench_corpus_memory
```

### Corpus translation cost (`CX0`/`CX1`)

```bash
# CX0 -- smoke: 5 patterns, S2ELab (the default source).
BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=CX0 \
  cargo bench --bench bench_corpus_translate

# CX1 -- full S2ELab sweep: every distinct pattern, no cap.
BENCH_RUN_LABEL=CX1 cargo bench --bench bench_corpus_translate
```

`CX1` covers S2ELab specifically, same as `CT1`/`CM1` -- the default
source when `BENCH_SOURCE` is unset. Run it once per other source
(own label each time, e.g. `BENCH_SOURCE=suricata BENCH_RUN_LABEL=CX2
...`) if a non-S2ELab sweep is ever needed; see "Translation cost"
under Quick reference above.

### Pathological time (`PT0`/`PT1`)

```bash
# PT0 -- smoke: n=5, p=5, 5 rows, 500ms cap.
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 \
  BENCH_RUN_LABEL=PT0 cargo bench --bench bench_pathological_time

# PT1 -- full pathological sweep per category: every row, every
# category, each row's own suggested n/p.
BENCH_RUN_LABEL=PT1 cargo bench --bench bench_pathological_time
```

### Pathological memory (`PM0`/`PM1`)

```bash
# PM0 -- smoke: n=5, p=5, 5 rows, 500ms cap.
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 \
  BENCH_RUN_LABEL=PM0 cargo bench --bench bench_pathological_memory

# PM1 -- full pathological sweep per category.
BENCH_RUN_LABEL=PM1 cargo bench --bench bench_pathological_memory
```

### Pathological translation cost (`PX0`/`PX1`)

```bash
# PX0 -- smoke: p=5, 5 rows.
PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=PX0 \
  cargo bench --bench bench_pathological_translate

# PX1 -- full pathological sweep per category: every row, each row's
# own suggested p (categories C/D/J/K are the ones actually worth
# reading here -- see bench_pathological_translate.rs's own section).
BENCH_RUN_LABEL=PX1 cargo bench --bench bench_pathological_translate
```
