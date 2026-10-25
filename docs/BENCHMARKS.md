# Benchmarks

Seven `cargo bench` targets. Two datasets, three axes each
(`bench_corpus_*` / `bench_pathological_*` x time / memory /
translate), plus one scoped comparison (`bench_stack_depth`):

| Target | Dataset | Measures |
|---|---|---|
| `bench_corpus_time` | S2ELab corpus | parser wall-clock time |
| `bench_corpus_memory` | S2ELab corpus | parser heap allocation |
| `bench_corpus_translate` | S2ELab corpus | translation time + memory + node count |
| `bench_pathological_time` | pathological stress patterns | parser wall-clock time |
| `bench_pathological_memory` | pathological stress patterns | parser heap allocation |
| `bench_pathological_translate` | pathological stress patterns | translation time + memory + node count |
| `bench_stack_depth` | pathological stress patterns (3 rows) | `deriv_std_rec` vs. `deriv_std_loop` stack-depth limit |

None use Criterion (see below). All are plain reporting binaries
(`harness = false`) that print a table and write it to a CSV under
`reports/benches/`.

`bench_corpus_*` reads `data/processed/s2-elab/prepared.jsonl` (see
[DATASET.md](DATASET.md) for how to build it). `bench_pathological_*`
and `bench_stack_depth` read `data/pathological/pathological_patterns.json`
directly, no preparation step.

`bench_corpus_time`, `bench_corpus_memory`, `bench_pathological_time`,
`bench_pathological_memory`, and `bench_stack_depth` each run every
entry in its own subprocess (so a pathological entry can be killed
outright and its memory reclaimed, rather than merely given up on) via
a shared worker. Build it first:

```bash
cargo build --release --example bench_worker            # for the four above
cargo build --release --example bench_stack_depth_worker # for bench_stack_depth only
```

Neither `*_translate` target needs a worker: both measure translation
(`core_regex`) only, which bounds itself via a depth/node cap before
the expensive part runs, so there's no runaway call to isolate.

## Why none of these use Criterion

Criterion assumes a cheap closure measured many times. A corpus pass
takes seconds to hours per parser; sampling it 20+ times multiplies
runtime for no statistical gain, and by the time a long sample set
finishes, thermal drift and OS scheduling dominate the spread Criterion
is trying to characterize anyway. This project used to carry two
Criterion targets (`bench_match`, `bench_parse`) over hand-built
synthetic ASTs, bypassing the real frontend; both were removed (the
pathological dataset's categories now cover the same shapes through
real string parsing, more representatively), and `criterion` with them.

## Configuration

Every target reads its configuration from the environment -- no flags,
no CLI arguments.

| Variable | Applies to | Values | Default | Notes |
|---|---|---|---|---|
| `BENCH_SOURCE` | `bench_corpus_translate` only | `suricata`/`snort`, `spamassassin`, `regexlib`, `s2-elab`, `innovatorved` (comma-separated) | S2ELab | Only source with real prepared data; others accept patterns but have no given-input extraction wired in |
| `BENCH_PATTERN_LIMIT` | all except `bench_stack_depth` | positive integer | unset (no cap) | Caps distinct patterns (corpus targets) or selected *rows*, before the `n`/`p` sweep (pathological targets) |
| `BENCH_MATCH_INTENT` | `bench_corpus_*` only | `direct`, `explicit`, `both` | `explicit` | `explicit` = `ExplicitFullMatch` (`^...$`) only; `direct` = `UnanchoredAmbiguous` only; see [DATASET.md](DATASET.md) |
| `BENCH_SPLIT_OUTCOME` | `bench_corpus_time` only | `1`/`0` etc. | `0` | Adds a second table splitting time by match/non-match |
| `BENCH_ENTRY_TIMEOUT_MS` | `bench_corpus_time`/`memory`, `bench_pathological_time`/`memory` | positive integer (ms) | `2000` | Per-entry cap; exceeding it kills the worker process (`timed_out`) rather than blocking the run |
| `BENCH_MEM_CAP_MB` | `bench_corpus_time`/`memory`, `bench_pathological_time`/`memory` | positive integer (MiB) | unset (unbounded) | Per-entry virtual-memory ceiling on the worker (`setrlimit(RLIMIT_AS, ..)`, Unix only); exceeding it is a contained `crashed`, never a system-wide OOM -- see below |
| `BENCH_RUN_LABEL` | all except `*_translate`'s aggregate CSV and `bench_stack_depth` | filename-safe string | unset (bare filename) | Suffixes the output filename, so different configs don't overwrite each other |
| `BENCH_TRANSLATE_RENDER_MAX_NODES` | `bench_corpus_translate` only | positive integer | `150` | Patterns above this node count get a placeholder instead of a rendered tree in `_patterns.csv` |
| `BENCH_TRANSLATE_EXAMPLES` | `bench_corpus_translate` only | non-negative integer | `5` | How many smallest distinct patterns per source to fully render to `translation_examples.txt` |
| `PATHO_JSON_PATH` | `bench_pathological_*`, `bench_stack_depth` (as `STACKDEPTH_JSON_PATH`) | file path | `data/pathological/pathological_patterns.json` | |
| `PATHO_SECTION` | `bench_pathological_*` | `pathological`, `baseline`, `generated`, `all` | `all` | Restricts to one of the dataset's three arrays |
| `PATHO_CATEGORY` | `bench_pathological_*` | comma-separated `A`-`L` | unset (every category present) | |
| `PATHO_ROW_ID` | `bench_pathological_time`/`memory` | comma-separated row ids | unset (every row other filters admit) | Narrow to one row + sweep `PATHO_N` for that pattern's own (input size, time) curve |
| `PATHO_N` | `bench_pathological_*` | comma-separated integers | each row's `suggested_n` (fallback 20) | For `bench_pathological_translate`: only moves a row whose *pattern* itself is `n`-templated with no `generator` (category D); every other row's translated form never depends on input length |
| `PATHO_P` | `bench_pathological_*` | comma-separated integers | each row's `suggested_p` (fallback 100) | Only matters for a `generated_patterns` row with a `generator` |
| `STACKDEPTH_ROW_IDS` | `bench_stack_depth` only | comma-separated row ids | `P01,P30,E51` | |
| `STACKDEPTH_TIMEOUT_MS` | `bench_stack_depth` only | positive integer (ms) | `15000` | Raised well past the other targets' 2000ms default -- see its own section |
| `STACKDEPTH_N_START` / `STACKDEPTH_N_CAP` | `bench_stack_depth` only | positive integer | `1` / `2000000` | Sweep doubles N from start until crash/timeout/cap |

A few variables need more than a one-liner:

- **`BENCH_ENTRY_TIMEOUT_MS`** -- a `timed_out` entry's worker is
  *killed*, not merely abandoned: a killed process reclaims its
  memory, a thread merely stopped waiting on does not (an earlier
  thread-based version OOM-killed a whole run; see
  `bench_corpus_time.rs`'s module doc for the full account). Cost: wall
  clock pays roughly `N_timed_out x cap`, since killing itself blocks.
- **`BENCH_MEM_CAP_MB`** -- unset (the default) is unbounded, same
  behaviour as before this existed: a single entry with no ceiling of
  its own can in principle exhaust the whole machine's memory, which
  is exactly what the OS's OOM killer can then resolve by picking *any*
  process as its victim, not just the offending worker. Setting this
  caps the worker's own virtual address space before the parser call;
  exceeding it aborts that one process by signal (Rust's own
  allocator-failure handler), indistinguishable from any other
  `crashed` entry to the caller -- no new outcome category. A cap set
  below what even translation plus the worker's own 256 MiB stack
  thread need fails immediately, which is the correct, honest result
  for too low a cap, not a bug.
- **`BENCH_RUN_LABEL`** -- works as a filename suffix everywhere except
  `bench_corpus_translate.csv`/`bench_pathological_translate.csv`,
  whose aggregate CSVs are always exactly that name, **appended** to
  rather than overwritten (so a run scoped to one source/category and
  a run covering everything both add rows to the same file); the label
  is instead recorded as a `run_label` column on each row. Their
  `_patterns.csv` catalogue still uses it as a filename suffix like
  everywhere else. `bench_stack_depth` doesn't read this variable at
  all -- it always writes the same `bench_stack_depth.csv`, no suffix.
- **`PATHO_ROW_ID`**: `bench_pathological_time`/`memory` only, not
  `bench_pathological_translate` -- the latter's own selection reuses
  `PATHO_CATEGORY`/`BENCH_PATTERN_LIMIT` but has no row-id filter today.

If filtering loads nothing, these targets panic naming the active
filters.

## `bench_corpus_time.rs`

```bash
cargo bench --bench bench_corpus_time
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_corpus_time
BENCH_SPLIT_OUTCOME=1 cargo bench --bench bench_corpus_time
```

S2ELab only. One pass, each entry timed individually in its own
worker. Writes `reports/benches/bench_corpus_time.csv`. Per parser:
`measured` (neither timed out nor crashed -- what the distribution
columns are computed over), `timed_out`, `crashed` (worker died by
signal, e.g. a stack overflow, before the cap), `wall (s)` (this
process's own time around the whole loop, including spawn/pipe
overhead -- not the sum of the columns below), `mean`/`median`/`p99`/
`max (ms)` (over `measured` entries only; `max` is a lower bound on the
corpus's true worst case, since the entries likeliest to time out are
exactly the ones excluded). `mean` is a macro-average -- each
*pattern*'s own mean first, then an unweighted average of those
per-pattern means -- so a pattern with many given examples does not
outweigh one with few (patterns are no longer truncated to a common
count; see `docs/DATASET.md`); `median`/`p99`/`max` stay flat over
every measured input, a different question ("how bad does the worst
single input get"). With `BENCH_SPLIT_OUTCOME=1`, a second table
splits time by match vs. non-match.

Also writes `..._entries.csv`: one row per individual
`source,parser,pattern,input,expected_match,outcome,micros,
encode_micros,derive_micros,decode_micros` measurement, with the
actual pattern/input text (truncated past 200 characters) -- filter to
one `pattern` and compare across parsers for that single real
pattern's own numbers, same role as `bench_pathological_time.rs`'s own
entries file.

## `bench_corpus_memory.rs`

```bash
cargo bench --bench bench_corpus_memory
```

The heap-allocation counterpart to `bench_corpus_time` -- same source,
same worker, same timeout/crash accounting, reporting bytes instead of
durations. "Heap" specifically: the counting `#[global_allocator]`
(living in `bench_worker`, not here) sees `GlobalAlloc` traffic only, so
a cost dominated by stack depth rather than allocation won't show up.
Writes `reports/benches/bench_corpus_memory.csv`: `total bytes` (every
allocation including freed-and-reallocated churn, over `measured`
entries), `avg/entry` (a macro-average -- each pattern's own mean
total-bytes first, then an unweighted average of those per-pattern
means, same reasoning as `bench_corpus_time.rs`'s `mean`), `max peak`
(highest live-byte high-water mark on any single entry, flat, not
macro-averaged). Also writes `..._entries.csv`, one row per
individual `source,parser,pattern,input,outcome,total_bytes,peak_bytes`
measurement, same role and convention as `bench_corpus_time.rs`'s own
entries file above. The three partial-derivative parsers (`deriv_bc`,
`pderiv_bc`, `pderiv_std`) allocate noticeably less than the two
standard ones -- `deriv_std` retains its whole derivation sequence,
`deriv_bc` reassigns one `ARegex` in place.

## `bench_corpus_translate.rs`

No worker needed.

```bash
cargo bench --bench bench_corpus_translate
BENCH_SOURCE=suricata cargo bench --bench bench_corpus_translate
```

Cost of frontend translation (PCRE string -> `Regex`) only, measured
once per *distinct pattern* (translation cost depends only on pattern
text, not the inputs it carries). No process isolation here, unlike
every other target: every pattern already passed `core_regex` once
during `dataset_prepare`, which enforces its own depth/node cap before
the expensive part runs, so nothing here can blow up that didn't
already fail to get this far; a per-pattern `catch_unwind` stands in
for crash containment instead.

Three outputs:
- `reports/benches/bench_corpus_translate.csv` -- one row per source
  (mean/median/p99/max of time, allocation, node count over
  successfully `translated` patterns; `rejected`/`crashed` excluded).
  **Appended**, not overwritten -- see `BENCH_RUN_LABEL` above.
- `reports/benches/bench_corpus_translate{label}_patterns.csv` -- one
  row per distinct pattern, with the translated `Regex`'s own `Debug`
  rendering when under `BENCH_TRANSLATE_RENDER_MAX_NODES`. The
  catalogue to hand-pick thesis examples from.
- `reports/demos/translation_examples.txt` -- the
  `BENCH_TRANSLATE_EXAMPLES` smallest patterns per source, fully
  rendered. Never labelled.

## `bench_pathological_time.rs`

```bash
cargo bench --bench bench_pathological_time
PATHO_CATEGORY=A,B cargo bench --bench bench_pathological_time

# One pattern's own scaling curve (a Fig.-7-style line-chart source):
PATHO_ROW_ID=P01 PATHO_N=20,50,100,200,500,1000 cargo bench --bench bench_pathological_time

# The literal FLOPS'14 Fig. 7 reproduction (rows P60/P61, see below):
PATHO_ROW_ID=P60 PATHO_N=1000000,5000000,10000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_RUN_LABEL=FIG7A cargo bench --bench bench_pathological_time
PATHO_ROW_ID=P61 PATHO_N=500000,2500000,5000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_RUN_LABEL=FIG7B cargo bench --bench bench_pathological_time
```

Entries are grouped by category (A-L), reported the same way as
`bench_corpus_time`. Reuses `bench_worker` and its accounting
unchanged, with one difference: a pathological pattern that fails to
translate at all (hits `core_regex`'s own cap) is left to show up as
`crashed` here rather than filtered out beforehand -- a real outcome
worth seeing for categories swept to a large `p`.

Writes `reports/benches/bench_pathological_time.csv` (category/parser
summary) and `..._entries.csv` (one row per individual
`row_id,category,parser,n,p,side,pattern,input,outcome,micros,
encode_micros,derive_micros,decode_micros` measurement -- the
summary's raw material, with the actual pattern/input text used,
truncated past 200 characters). Filter the entries file to one
`row_id`+`parser` and sort by `n` for a single pattern's (input size,
time) curve.

**Rows `P60`/`P61`: the literal Fig. 7 reproduction.** `(a|b|ab)*` at
the paper's own scale -- `P60` sweeps `'a'`-repeated input (paper's
Fig. 7a range, 1-10M), `P61` sweeps `'ab'`-repeated input (Fig. 7b
range, 0.5-5M). Measured (`'a'`-repeated, isolated per-parser runs):

| parser | 1M | 5M | 10M |
|---|---|---|---|
| `deriv_std_rec` | crashes (stack overflow) | crashes | crashes |
| `deriv_std_loop` | OOM-killed (~110s in) | not tested (already fails at 1M) | -- |
| `deriv_bc` | 7.2s, 249 MB peak | 40.7s, 1.32 GB peak | 82.9s, 2.64 GB peak |
| `pderiv_bc` | 6.0s, 141 MB peak | 26.7s, 778 MB peak | 53.4s, 1.56 GB peak |
| `pderiv_std` | 5.3s, 233 MB peak | 26.4s, 1.24 GB peak | 63.8s, 2.48 GB peak |

The three bit-coded/partial-derivative parsers scale linearly in both
time and memory across the full 10M-character range; `deriv_std_rec`/
`deriv_std_loop` fail outright, as expected -- they implement the
paper's Fig. 3 algorithm, defined with no simplification step at all,
so every Star iteration's dead alternatives accumulate without bound.

**`parseBC` vs. `DERIV`: what Fig. 7 actually measures.** FLOPS'14
benchmarks two *different* implementations, not one. `parseBC` (Fig.
5/6) is the general bit-coded algorithm building the full ambiguous
parse tree -- what `deriv_bc`/`pderiv_bc` implement here. `DERIV` (the
one actually plotted in Fig. 7) is a separate, narrower, DFA-compiled
implementation for standard POSIX submatching that keeps only each
repeated subexpression's *last* iteration -- an easier problem, solved
by code the paper never open-sourced. So Fig. 7's curve is not a
same-task comparison for any parser here; the table above (reaching
the same 10M-character *scale* without crashing) is the closest
same-task analogue available, not a throughput-parity claim.

**Why `deriv_bc` used to fail at this scale.** Two stacked bugs, fixed
in order: (1) `simp_once`'s `Seq`/`Alt` collapse cases called
`.to_vec()` on a Star's accumulated bit-code history on every
character -- an O(k) read repeated for k=1..n, i.e. O(n^2) total.
Fixed by `Bits::prepend_bits`/`fuse_bits`, an O(1) splice that keeps
the history an unmaterialized chain until the final decode. (2) With
that fixed, a second, previously-unreachable bug surfaced:
`parse_bitcoded_recursive`'s driver recursed once per input character
with no tail-call elimination, stack-overflowing in the low millions.
Fixed by replacing it with a plain loop
(`derive_and_simplify`, `src/parsers/deriv_bc/parse.rs`).
`pderiv_bc` never had either bug -- its own hot path already
materializes its history once at the end and loops rather than
recurses.

## `bench_pathological_memory.rs`

```bash
cargo bench --bench bench_pathological_memory
```

The heap-allocation counterpart to `bench_pathological_time`: same
dataset, grouping, sweep, `PATHO_*` variables, worker, and timeout/
crash accounting, reporting `total_bytes`/`avg_bytes`/`max_peak`
instead of durations. Writes `reports/benches/bench_pathological_memory.csv`
and `..._entries.csv` (same per-entry shape as the time target's,
with `total_bytes`/`peak_bytes` in place of the micros columns).

## `bench_pathological_translate.rs`

No worker needed.

```bash
cargo bench --bench bench_pathological_translate
PATHO_CATEGORY=C,D,J,K cargo bench --bench bench_pathological_translate
PATHO_SECTION=generated PATHO_P=100000 cargo bench --bench bench_pathological_translate
```

The translation-only counterpart to `bench_pathological_time`/
`memory`. Why it exists: categories **C**/**D**/**J**/**K** are built
around compile/translate blow-up (`{m,n}`-unrolling, large
alternation/group trees) as their own `stress_axis`, but the other two
pathological targets only measure the parser call on an
already-translated `Regex` -- so translation cost itself, the actual
axis those categories stress, was never measured anywhere else. Sweeps
`PATHO_P` for a `generated_patterns` row with a `generator` (J/K), or
`PATHO_N` for a `pattern_template` row with none -- category D's own
unrolled quantifier bound is `n`, not `p`, so this is the only other
target where `PATHO_N` does anything; every other row's translated
pattern depends on neither and is measured once. Writes
`reports/benches/bench_pathological_translate.csv`, one row per
category (same `translated`/`rejected`/`crashed` split as
`bench_corpus_translate`). **Appended**, like
`bench_corpus_translate.csv`. Also writes
`..._patterns.csv` (one row per distinct row/`p`/`n` entry, same
catalogue role as the other two pathological targets' own entries
files), overwritten each run.

## `bench_stack_depth.rs`

```bash
cargo build --release --example bench_stack_depth_worker   # build once
cargo bench --bench bench_stack_depth                       # default: P01, P30, E51
STACKDEPTH_ROW_IDS=E51 cargo bench --bench bench_stack_depth # just the row that isolates the difference
```

Compares the one pair in this engine where the same algorithm has two
implementations whose *stack* behaviour differs: `deriv_std_rec`
(native recursion, one stack frame per input character) vs.
`deriv_std_loop` (an explicit `Vec`, no such recursion) -- every other
parser is out of scope. For each (row, parser), N doubles from
`STACKDEPTH_N_START` until the worker is killed by a signal
(`crashed`), a probe exceeds `STACKDEPTH_TIMEOUT_MS` (`timed_out`), or
N exceeds `STACKDEPTH_N_CAP` (`reached_sweep_cap`); the last clean N is
`max_n_survived`. Writes `reports/benches/bench_stack_depth.csv`
(`row_id,category,parser,max_n_survived,outcome,next_n,elapsed_s,detail`).

Default rows, picked by hand: **P01** `(a+)+` and **P30** `(\w+)` x10
both hit a *shared* derivative-size blowup wall (inside `deriv()`,
called by both parsers alike) at a low N, well before either
implementation's stack depth would matter -- for these two, "both hit
the same wall at the same N" is the honest, expected result, not a
stack-depth finding. **E51** (pattern `x`, swept input `a*N`) is the
row that actually isolates the difference: it sends the derivative
straight to a tiny fixed-size residual, so there's no size blowup to
hide behind. `deriv_std_loop` clears 2,000,000 characters in ~0.06s
(effectively O(n), no crash risk); `deriv_std_rec` on the identical
input survives only ~55,000 characters (~4.9s) and genuinely
stack-overflows at 60,000 -- a real killed process, not a timeout.
`STACKDEPTH_TIMEOUT_MS` defaults to 15000 (not 2000) specifically so
this crash surfaces as `crashed` rather than a false `timed_out`:
`parse_recursive_helper` also rebuilds the whole remaining input
string on every recursive call, an O(n^2) cost by itself, which is why
55,000 plain characters already takes ~5s -- a separate, real
inefficiency worth fixing on its own merits, out of scope here.

## Choosing a target

| Question | Target |
|---|---|
| Does the corpus read correctly? | `bench_corpus_time` with `BENCH_PATTERN_LIMIT=5` |
| How long does each parser take over the whole corpus? | `bench_corpus_time` |
| Which patterns dominate the cost? | `bench_corpus_time` (median vs. p99 vs. max) |
| Do matches and non-matches cost differently? | `bench_corpus_time` with `BENCH_SPLIT_OUTCOME=1` |
| How much memory does each parser allocate? | `bench_corpus_memory` |
| How much does translation itself cost? | `bench_corpus_translate` |
| What does a real pattern's translated form look like? | `bench_corpus_translate`'s `_patterns.csv` or `translation_examples.txt` |
| How does a parser scale on a known-pathological pattern, by category? | `bench_pathological_time` |
| How much does a pathological pattern allocate as it's swept larger? | `bench_pathological_memory` |
| How much does translation cost on a compile-blow-up pattern (C/D/J/K)? | `bench_pathological_translate` |
| Does `deriv_std_rec` stack-overflow before `deriv_std_loop` would? | `bench_stack_depth` |

## Running a full set

Each command below sets `BENCH_RUN_LABEL` so its CSV lands in its own
file under `reports/benches/` instead of overwriting another run.
Naming: `<dataset><axis><tier>` -- `C`/`P` for corpus/pathological,
`T`/`M`/`X` for time/memory/translate, `0`/`1` for smoke/full sweep.
The two `*_translate` aggregate CSVs still record the label as a
`run_label` column rather than renaming the file (see `BENCH_RUN_LABEL`
above). `bench_stack_depth` reads no such variable and always
overwrites `bench_stack_depth.csv` -- its two commands below (labelled
`SD0`/`SD1` here only for reference, not as a real filename suffix)
must be run and the CSV saved aside before the other, if both results
are wanted.

```bash
# Corpus time: CT0 smoke, CT1 full sweep with match/non-match split.
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CT0 \
cargo bench --bench bench_corpus_time

BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=CT1 cargo bench --bench bench_corpus_time


# Corpus memory: CM0 smoke, CM1 full sweep.
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CM0 \
cargo bench --bench bench_corpus_memory

BENCH_RUN_LABEL=CM1 cargo bench --bench bench_corpus_memory


# Corpus translate: CX0 smoke, CX1 full sweep (S2ELab; repeat per other source with its own label).
BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=CX0 \
cargo bench --bench bench_corpus_translate

BENCH_RUN_LABEL=CX1 cargo bench --bench bench_corpus_translate



# Pathological time: PT0 smoke, PT1 full sweep (every row/category, each row's own suggested n/p).
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 BENCH_RUN_LABEL=PT0 \
cargo bench --bench bench_pathological_time

BENCH_RUN_LABEL=PT1 cargo bench --bench bench_pathological_time
# PT1 now does real work at each category's calibrated suggested_n
# (see DATASET.md's Categories table) rather than stopping short of
# deriv_std's own collapse -- expect several minutes per blow-up
# category (A/B/C/D/E/F/H), dominated by every non-crashing entry
# sitting out the default 2000ms BENCH_ENTRY_TIMEOUT_MS before giving
# up. For a faster, coarser full sweep, lower that cap (e.g. the 500ms
# PT0 already uses above) at the cost of a timed-out entry that might
# otherwise have measured.



# Pathological memory: PM0 smoke, PM1 full sweep.
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 BENCH_RUN_LABEL=PM0 \
cargo bench --bench bench_pathological_memory

BENCH_RUN_LABEL=PM1 cargo bench --bench bench_pathological_memory


# Pathological translate: PX0 smoke, PX1 full sweep.
PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=PX0 \
cargo bench --bench bench_pathological_translate

BENCH_RUN_LABEL=PX1 cargo bench --bench bench_pathological_translate


# Stack depth: SD0 smoke (tiny cap, just row E51), SD1 full 
STACKDEPTH_ROW_IDS=E51 STACKDEPTH_N_CAP=1000 STACKDEPTH_TIMEOUT_MS=2000 \
cargo bench --bench bench_stack_depth
```
