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

### Variable notes

- **`BENCH_SOURCE`** — Only S2ELab has prepared input data; other sources accept patterns only.
- **`BENCH_PATTERN_LIMIT`** — Limits corpus patterns or pathological rows before `n`/`p` sweeps.
- **`BENCH_MATCH_INTENT`** — `explicit`: anchored `^...$` patterns only. `direct`: `UnanchoredAmbiguous` only. See [DATASET.md](DATASET.md).
- **`BENCH_SPLIT_OUTCOME`** — Adds separate match/non-match timing tables.
- **`BENCH_ENTRY_TIMEOUT_MS`** — Per-entry timeout; kills the worker and reclaims its memory. Runtime overhead is roughly `timed_out entries × timeout`.
- **`BENCH_MEM_CAP_MB`** — Per-worker virtual-memory limit (Unix `RLIMIT_AS`). Exceeding it reports `crashed`. Unset means unlimited, risking system-wide OOM. Too low a cap—including below translation needs plus the worker's 256 MiB stack—can cause every entry to fail.
- **`BENCH_RUN_LABEL`** — Adds filename suffixes and a `run_label` column, except translation aggregate CSVs, which append to fixed filenames. `bench_stack_depth` ignores it and always writes `bench_stack_depth.csv`.
- **`PATHO_ROW_ID`** — Filters rows for pathological time/memory benchmarks only, not translation.
- **`PATHO_N`** — For pathological translation, affects only `n`-templated patterns without a generator (category D).
- **`PATHO_P`** — Affects only `generated_patterns` rows with a generator.
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


### Representative Pathological Cases: Timing

Each `PATHO_N` list below mixes small points where `deriv_std_rec`/`deriv_std_loop` still complete (confirmed by probing each row's own entries CSV for exactly where `outcome` flips from `measured` to `timed_out`) with the original large/paper-scale points for the other three parsers. That way every single-row run reports a real number for all five parsers, not just the ones fast enough for the paper-scale input.

```bash
# REPRESENTATIVE PATHOLOGICAL CASES: TIMING

# P60 (E): (a|b|ab)*, 'a'-repeated.
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological_time

# P61 (E): (a|b|ab)*, 'ab'-repeated.
PATHO_ROW_ID=P61 PATHO_N=5,10,15,500000,2500000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological_time

# P20 (E): (a|b|ab)+.
PATHO_ROW_ID=P20 PATHO_N=5,10,13,16,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7C \
  cargo bench --bench bench_pathological_time

# P01 (A): (a+)+.
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,10000,30000,50000,200000,1000000 \
  BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_time

# P09 (B): (a|aa)+.
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100000,500000,1000000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_time

# P25/P27/P28 (F): (a|b)*c / ([ab]+)*c / ([ab]*)*c.
PATHO_ROW_ID=P25,P27,P28 PATHO_N=1,3,7,10,50,300,1000,5000,10000,30000,50000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F \
  cargo bench --bench bench_pathological_time
```

These are starting points. Raise or lower timeouts and caps once real run costs on the target machine are known.



## bench_pathological_memory

```bash
cargo bench --bench bench_pathological_memory
```

Same dataset, grouping, sweep, `PATHO_*` variables, worker, and timeout and crash accounting as `bench_pathological_time`. Reports `total_bytes`, `avg_bytes`, and `max_peak` instead of durations. Writes `reports/benches/bench_pathological_memory.csv` and `..._entries.csv`, with `total_bytes` and `peak_bytes` in place of the micros columns.


### Representative Pathological Cases: Memory

Same six representatives and the same `PATHO_N` lists as the time section above, so each row's memory curve covers the same small points where `deriv_std_rec`/`deriv_std_loop` complete, plus the large/paper-scale points for the other three parsers.

```bash
# REPRESENTATIVE PATHOLOGICAL CASES: MEMORY

# P60 (E): (a|b|ab)*, 'a'-repeated.
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological_memory

# P61 (E): (a|b|ab)*, 'ab'-repeated.
PATHO_ROW_ID=P61 PATHO_N=5,10,15,500000,2500000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological_memory

# P20 (E): (a|b|ab)+.
PATHO_ROW_ID=P20 PATHO_N=5,10,13,16,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7C \
  cargo bench --bench bench_pathological_memory

# P01 (A): (a+)+.
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,10000,30000,50000,200000,1000000 \
  BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_memory

# P09 (B): (a|aa)+.
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100000,500000,1000000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_memory

# P25/P27/P28 (F): (a|b)*c / ([ab]+)*c / ([ab]*)*c.
PATHO_ROW_ID=P25,P27,P28 PATHO_N=1,3,7,10,50,300,1000,5000,10000,30000,50000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F \
  cargo bench --bench bench_pathological_memory
```

These are starting points. Raise or lower timeouts and caps once real run costs on the target machine are known.


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



## Quick reference

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