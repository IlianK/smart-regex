# Benchmarks

Seven `cargo bench` targets. Two datasets, three axes each (corpus and pathological against time, memory, and translate), plus one scoped comparison.

| Target | Dataset | Measures |
|---|---|---|
| `bench_corpus_time` | S2ELab corpus | parser wall-clock time |
| `bench_corpus_memory` | S2ELab corpus | parser heap allocation |
| `bench_corpus_translate` | S2ELab corpus | translation time, memory, node count |
| `bench_pathological_time` | pathological stress patterns | parser wall-clock time |
| `bench_pathological_memory` | pathological stress patterns | parser heap allocation |
| `bench_pathological_translate` | pathological stress patterns | translation time, memory, node count |
| `bench_stack_depth` | pathological stress patterns (3 rows) | `deriv_std_rec` vs `deriv_std_loop` stack-depth limit |

None use Criterion. All seven are plain reporting binaries (`harness = false`) that print a table and write a CSV under `reports/benches/`.

`bench_corpus_*` reads `data/processed/s2-elab/prepared.jsonl`. See [DATASET.md](DATASET.md) for how to build it. `bench_pathological_*` and `bench_stack_depth` read `data/pathological/pathological_patterns.json` directly, no preparation step.

Five targets run every entry in its own subprocess so a pathological entry can be killed and its memory reclaimed, by using two workers:.

```bash
cargo build --release --example bench_worker              # for the four time/memory targets
cargo build --release --example bench_stack_depth_worker  # for bench_stack_depth only
```

Neither `*_translate` target needs a worker. Both measure `core_regex` only, which bounds itself via a depth and node cap before the expensive part runs.

## Configuration

Every target reads configuration from the environment. No flags, no CLI arguments.

| Variable | Applies to | Values | Default |
|---|---|---|---|
| `BENCH_SOURCE` | `bench_corpus_translate` only | `suricata`, `snort`, `spamassassin`, `regexlib`, `s2-elab`, `innovatorved` (comma-separated) | S2ELab |
| `BENCH_PATTERN_LIMIT` | all except `bench_stack_depth` | positive integer | unset (no cap) |
| `BENCH_MATCH_INTENT` | `bench_corpus_*` only | `direct`, `explicit`, `both` | `explicit` |
| `BENCH_SPLIT_OUTCOME` | `bench_corpus_time` only | `1` or `0` | `0` |
| `BENCH_ENTRY_TIMEOUT_MS` | corpus and pathological time/memory | positive integer (ms) | `2000` |
| `BENCH_MEM_CAP_MB` | corpus and pathological time/memory | positive integer (MiB) | unset (unbounded) |
| `BENCH_RUN_LABEL` | all except translate aggregates and `bench_stack_depth` | filename-safe string | unset |
| `BENCH_TRANSLATE_RENDER_MAX_NODES` | `bench_corpus_translate` only | positive integer | `150` |
| `BENCH_TRANSLATE_EXAMPLES` | `bench_corpus_translate` only | non-negative integer | `5` |
| `PATHO_JSON_PATH` | `bench_pathological_*`, `bench_stack_depth` (as `STACKDEPTH_JSON_PATH`) | file path | `data/pathological/pathological_patterns.json` |
| `PATHO_SECTION` | `bench_pathological_*` | `pathological`, `baseline`, `generated`, `all` | `all` |
| `PATHO_CATEGORY` | `bench_pathological_*` | comma-separated `A` to `L` | unset (every category) |
| `PATHO_ROW_ID` | `bench_pathological_time` and `memory` | comma-separated row ids | unset |
| `PATHO_N` | `bench_pathological_*` | comma-separated integers | each row's `suggested_n`, fallback 20 |
| `PATHO_P` | `bench_pathological_*` | comma-separated integers | each row's `suggested_p`, fallback 100 |
| `STACKDEPTH_ROW_IDS` | `bench_stack_depth` only | comma-separated row ids | `P01,P30,E51` |
| `STACKDEPTH_TIMEOUT_MS` | `bench_stack_depth` only | positive integer (ms) | `15000` |
| `STACKDEPTH_N_START` and `STACKDEPTH_N_CAP` | `bench_stack_depth` only | positive integers | `1` and `2000000` |

Short notes on the more involved variables.

- **`BENCH_SOURCE`.** S2ELab is the only source with real prepared data. The other four accept patterns but have no given-input extraction wired in.
- **`BENCH_PATTERN_LIMIT`.** Caps distinct patterns for the corpus targets. Caps selected rows before the `n` and `p` sweep for the pathological targets.
- **`BENCH_MATCH_INTENT`.** `explicit` selects `ExplicitFullMatch` (top-level `^...$`) only. `direct` selects `UnanchoredAmbiguous` only. See [DATASET.md](DATASET.md).
- **`BENCH_SPLIT_OUTCOME`.** Adds a second table splitting time by match and non-match.
- **`BENCH_ENTRY_TIMEOUT_MS`.** Per-entry cap. Exceeding it kills the worker process. A killed process reclaims its memory. A thread stopped waiting on does not. Wall-clock cost is roughly `N_timed_out x cap`, because killing itself blocks.
- **`BENCH_MEM_CAP_MB`.** Sets a per-entry virtual-memory ceiling on the worker, via `setrlimit(RLIMIT_AS, ..)` on Unix. Exceeding it aborts that one process by signal, which the caller reports as `crashed`. No new outcome category. Unset means unbounded, so a single entry can exhaust the whole machine's memory. The OS OOM killer can then pick any process as its victim. A cap below translation plus the worker's own 256 MiB stack thread fails immediately. Too low a cap returns that failure for every entry.
- **`BENCH_RUN_LABEL`.** Works as a filename suffix everywhere except the two translate aggregate CSVs. Those always use the same filename and are appended rather than overwritten. The label is stored as a `run_label` column on each row. `bench_stack_depth` reads no such variable and always writes `bench_stack_depth.csv`.
- **`PATHO_ROW_ID`.** `bench_pathological_time` and `memory` only. `bench_pathological_translate` reuses `PATHO_CATEGORY` and `BENCH_PATTERN_LIMIT` but has no row-id filter.
- **`PATHO_N`.** For `bench_pathological_translate`, only a row whose pattern is itself `n`-templated with no `generator` (category D) responds. Every other row's translated form does not depend on input length.
- **`PATHO_P`.** Only a `generated_patterns` row with a `generator` responds.

If filtering loads nothing, these targets panic naming the active filters.

## bench_corpus_time

```bash
cargo bench --bench bench_corpus_time
BENCH_PATTERN_LIMIT=5 cargo bench --bench bench_corpus_time
BENCH_SPLIT_OUTCOME=1 cargo bench --bench bench_corpus_time
```

S2ELab only. One pass. Each entry timed in its own worker. Writes `reports/benches/bench_corpus_time.csv`.

Per-parser summary columns.

| Column | Meaning |
|---|---|
| `measured` | Neither timed out nor crashed. Distribution columns are computed over these. |
| `timed_out` | Worker exceeded `BENCH_ENTRY_TIMEOUT_MS`. |
| `crashed` | Worker died by signal before the cap. |
| `wall (s)` | This process's own time around the whole loop, including spawn and pipe overhead. |
| `mean` | Macro-average. Each pattern's own mean first, then an unweighted average of those per-pattern means. A pattern with many examples does not outweigh one with few. |
| `median` | Flat over every measured input. |
| `p99` | Flat over every measured input. |
| `max (ms)` | Flat over every measured input. A lower bound on the corpus's true worst case, since the entries likeliest to time out are excluded. |

`mean` answers the per-pattern question. `median`, `p99`, and `max` answer the per-input question.

With `BENCH_SPLIT_OUTCOME=1`, a second table splits time by match and non-match.

Also writes `..._entries.csv`. One row per measurement, with columns `source,parser,pattern,input,expected_match,outcome,micros,encode_micros,derive_micros,decode_micros`. Pattern and input text included, truncated past 200 characters. Filter to one `pattern` and compare across parsers.

## bench_corpus_memory

```bash
cargo bench --bench bench_corpus_memory
```

Same source, worker, timeout, and crash accounting as `bench_corpus_time`. Reports bytes instead of durations. Writes `reports/benches/bench_corpus_memory.csv`.

The counting `#[global_allocator]` lives in `bench_worker`. It sees `GlobalAlloc` traffic only. A cost dominated by stack depth rather than allocation does not show up.

Summary columns.

| Column | Meaning |
|---|---|
| `total bytes` | Every allocation including freed-and-reallocated churn, over `measured` entries. |
| `avg/entry` | Macro-average, same reasoning as `bench_corpus_time`'s `mean`. |
| `max peak` | Highest live-byte high-water mark on any single entry. Flat, not macro-averaged. |

Also writes `..._entries.csv`, one row per measurement, with columns `source,parser,pattern,input,outcome,total_bytes,peak_bytes`.

`deriv_std_rec` and `deriv_std_loop` allocate noticeably more than `deriv_bc`, `pderiv_bc`, and `pderiv_std`. `deriv_std` retains its whole derivation sequence. `deriv_bc` reassigns one `ARegex` in place.

## bench_corpus_translate

No worker needed.

```bash
cargo bench --bench bench_corpus_translate
BENCH_SOURCE=snort cargo bench --bench bench_corpus_translate
```

Frontend translation cost only, once per distinct pattern. Translation depends only on pattern text.

Every pattern already passed `core_regex` during `dataset_prepare`, which enforces its own depth and node cap before the expensive part runs. A per-pattern `catch_unwind` stands in for crash containment.

Three outputs.

| Output | Contents |
|---|---|
| `reports/benches/bench_corpus_translate.csv` | One row per source. Mean, median, p99, max of time, allocation, and node count, over `translated` patterns only. Appended, not overwritten. |
| `reports/benches/bench_corpus_translate{label}_patterns.csv` | One row per distinct pattern, with the translated `Regex`'s own `Debug` rendering when under `BENCH_TRANSLATE_RENDER_MAX_NODES`. Catalogue to hand-pick thesis examples from. |
| `reports/demos/translation_examples.txt` | The `BENCH_TRANSLATE_EXAMPLES` smallest patterns per source, fully rendered. Never labelled. |

## bench_pathological_time

```bash
cargo bench --bench bench_pathological_time
PATHO_CATEGORY=A,B cargo bench --bench bench_pathological_time
```

Entries grouped by category (A to L). Reported the same way as `bench_corpus_time`, reusing `bench_worker`. A pathological pattern that fails to translate at all shows up as `crashed`, not filtered beforehand.

Writes `reports/benches/bench_pathological_time.csv` (category and parser summary) and `..._entries.csv`, one row per measurement, with columns `row_id,category,parser,n,p,side,pattern,input,outcome,micros,encode_micros,derive_micros,decode_micros`. Pattern and input text included, truncated past 200 characters.

Filter the entries file to one `row_id` and `parser`, sort by `n`, and you have a single pattern's (input size, time) curve.

### Per-row scaling and Fig 7 sweeps

```bash
# Single row's own scaling curve
PATHO_ROW_ID=P01 PATHO_N=20,50,100,200,500,1000 cargo bench --bench bench_pathological_time

# Fig. 7a reproduction, P60, 'a'-repeated input
PATHO_ROW_ID=P60 PATHO_N=1000000,5000000,10000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_RUN_LABEL=FIG7A cargo bench --bench bench_pathological_time

# Fig. 7b reproduction, P61, 'ab'-repeated input
PATHO_ROW_ID=P61 PATHO_N=500000,2500000,5000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_RUN_LABEL=FIG7B cargo bench --bench bench_pathological_time

# P20, `(a|b|ab)+`, paper scale, full Fig. 7 story with the + variant
PATHO_ROW_ID=P20 PATHO_N=1000000,5000000,10000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7C cargo bench --bench bench_pathological_time

# P01, `(a+)+`. Brackets deriv_bc's collapse point
PATHO_ROW_ID=P01 PATHO_N=10000,30000,50000,200000,1000000 \
  BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_time

# P09, same family as P01, pushed further to find deriv_bc's limit
PATHO_ROW_ID=P09 PATHO_N=100000,500000,1000000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_time

# P25, P27, P28. Category F. All three together to bracket deriv_bc's collapse
PATHO_ROW_ID=P25,P27,P28 PATHO_N=5000,10000,30000,50000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F \
  cargo bench --bench bench_pathological_time
```

These are starting points. Raise or lower timeouts and caps once real run costs on the target machine are known.

### Fig 7 reproduction results

Rows `P60` and `P61` reproduce FLOPS'14 Fig. 7 at the paper's own scale. `P60` sweeps `'a'`-repeated input. `P61` sweeps `'ab'`-repeated input.

Measured on `'a'`-repeated input, isolated per-parser runs.

| parser | 1M | 5M | 10M |
|---|---|---|---|
| `deriv_std_rec` | crashes (stack overflow) | crashes | crashes |
| `deriv_std_loop` | OOM-killed (~110s in) | not tested | not tested |
| `deriv_bc` | 7.2s, 249 MB peak | 40.7s, 1.32 GB peak | 82.9s, 2.64 GB peak |
| `pderiv_bc` | 6.0s, 141 MB peak | 26.7s, 778 MB peak | 53.4s, 1.56 GB peak |
| `pderiv_std` | 5.3s, 233 MB peak | 26.4s, 1.24 GB peak | 63.8s, 2.48 GB peak |

`deriv_bc`, `pderiv_bc`, and `pderiv_std` scale linearly in both time and memory across 10M characters. `deriv_std_rec` and `deriv_std_loop` fail. They implement the Fig. 3 algorithm, which has no simplification step. Every Star iteration's dead alternatives accumulate.

### parseBC vs DERIV

FLOPS'14 benchmarks two implementations.

- `parseBC` is the bit-coded general parse-tree algorithm. `deriv_bc` and `pderiv_bc` implement this.
- `DERIV` is a narrower DFA-compiled POSIX submatching implementation that keeps only each repeated subexpression's last iteration. This is the curve Fig. 7 plots.

The two solve different problems. The table above reaches the same 10M-character scale with `parseBC`.

### Why deriv_bc reaches this scale

Two design choices.

- Bits materialize lazily via `prepend_bits` and `fuse_bits`. History stays an unmaterialized chain until final decode.
- The per-character driver is a loop (`derive_and_simplify`), not recursion. No stack frame per character.

`pderiv_bc` materializes history once at the end and loops rather than recurses.

## bench_pathological_memory

```bash
cargo bench --bench bench_pathological_memory
```

Same dataset, grouping, sweep, `PATHO_*` variables, worker, and timeout and crash accounting as `bench_pathological_time`. Reports `total_bytes`, `avg_bytes`, and `max_peak` instead of durations. Writes `reports/benches/bench_pathological_memory.csv` and `..._entries.csv`, with `total_bytes` and `peak_bytes` in place of the micros columns.

## bench_pathological_translate

No worker needed.

```bash
cargo bench --bench bench_pathological_translate
PATHO_CATEGORY=C,D,J,K cargo bench --bench bench_pathological_translate
PATHO_SECTION=generated PATHO_P=100000 cargo bench --bench bench_pathological_translate
```

Categories C, D, J, and K stress compile and translate blow-up as their own stress axis. The other two pathological targets only measure the parser call on an already-translated `Regex`. This target measures translation cost itself.

Sweeps `PATHO_P` for a `generated_patterns` row with a `generator` (J and K). Sweeps `PATHO_N` for a `pattern_template` row with no generator (category D, where the unrolled quantifier bound is `n`, not `p`). Other rows are measured once.

Writes `reports/benches/bench_pathological_translate.csv`, one row per category, with a `translated` and `rejected` and `crashed` split. Appended, like `bench_corpus_translate.csv`. Also writes `..._patterns.csv`, one row per distinct row, `p`, and `n` entry. Overwritten each run.

## bench_stack_depth

```bash
cargo build --release --example bench_stack_depth_worker   # build once
cargo bench --bench bench_stack_depth                       # P01, P30, E51
STACKDEPTH_ROW_IDS=E51 cargo bench --bench bench_stack_depth # only the isolating row
```

Compares `deriv_std_rec` (native recursion, one stack frame per input character) to `deriv_std_loop` (an explicit `Vec`). Every other parser is out of scope.

For each (row, parser), N doubles from `STACKDEPTH_N_START` until one of three outcomes.

| Outcome | Trigger |
|---|---|
| `crashed` | Worker killed by a signal. |
| `timed_out` | Probe exceeds `STACKDEPTH_TIMEOUT_MS`. |
| `reached_sweep_cap` | N exceeds `STACKDEPTH_N_CAP`. |

The last clean N is `max_n_survived`. Writes `reports/benches/bench_stack_depth.csv` with columns `row_id,category,parser,max_n_survived,outcome,next_n,elapsed_s,detail`.

Default rows.

- **P01** `(a+)+` and **P30** `(\w+)` x10 hit a shared derivative-size blowup wall inside `deriv()`, at a low N. Both parsers reach it at the same N, before either implementation's stack depth matters. Both hitting the same wall at the same N is the expected result.
- **E51** isolates the difference. Pattern `x`, swept input `a*N`. The derivative reduces to a tiny fixed-size residual, so there is no size blowup to hide behind. `deriv_std_loop` clears 2,000,000 characters in ~0.06s. `deriv_std_rec` survives ~55,000 characters (~4.9s) and stack-overflows at 60,000.

`STACKDEPTH_TIMEOUT_MS` defaults to 15000 so this crash surfaces as `crashed`. `parse_recursive_helper` also rebuilds the whole remaining input on every recursive call, an O(n^2) cost. That is why 55,000 plain characters already takes ~5s.



## Running a full set

Corpus and pathological commands set `BENCH_RUN_LABEL` so each CSV lands in its own file under `reports/benches/`. Naming scheme is `<dataset><axis><tier>`. `C` or `P` for corpus or pathological. `T`, `M`, or `X` for time, memory, or translate. `0` or `1` for smoke or full sweep.

The two translate aggregate CSVs record the label as a `run_label` column rather than renaming the file.

Stack-depth commands read no `BENCH_RUN_LABEL` and always overwrite `bench_stack_depth.csv`. Save that CSV aside before running the next stack-depth command.


### Corpus

```bash
# BENCHMARKS: S2ELAB
cargo build --release --example bench_worker
```

#### SMOKE TESTS
```bash
# Time
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CT0 \
  cargo bench --bench bench_corpus_time

# Memory
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CM0 \
  cargo bench --bench bench_corpus_memory

# Translate
BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=CX0 \
  cargo bench --bench bench_corpus_translate
```

#### FULL CORPUS
```bash
# Time
BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=CT1 \
  cargo bench --bench bench_corpus_time

# Memory
BENCH_RUN_LABEL=CM1 cargo bench --bench bench_corpus_memory

# Translate
BENCH_RUN_LABEL=CX1 cargo bench --bench bench_corpus_translate
```


### Pathological

#### SMOKE TESTS

```bash
# Time
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 BENCH_RUN_LABEL=PT0 \
  cargo bench --bench bench_pathological_time

# Memory
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 BENCH_RUN_LABEL=PM0 \
  cargo bench --bench bench_pathological_memory

# Translate
PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=PX0 \
  cargo bench --bench bench_pathological_translate
```

#### FULL CORPUS

```bash
# Time
BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=PT1 \
  cargo bench --bench bench_pathological_time

# Memory
BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=PM1 \
  cargo bench --bench bench_pathological_memory

# Translate
BENCH_RUN_LABEL=PX1 cargo bench --bench bench_pathological_translate
```


### Stack depth

```bash
# SD0 smoke, tiny cap, just row E51
STACKDEPTH_ROW_IDS=E51 STACKDEPTH_N_CAP=1000 STACKDEPTH_TIMEOUT_MS=2000 \
cargo bench --bench bench_stack_depth
```