# Examples

Runnable programs under `examples/`, invoked with `cargo run --example <name>`.

| Area | Demos | What they show |
|---|---|---|
| **Matching** | `demo_match` | Naive, Brzozowski, Antimirov matchers side by side |
| **Parsing** | `demo_parse` | Any one of the five parsers, with optional diagnostics. Reuses the CLI's `--parser`, `--diag`, `--diag-report` names (see [CLI.md](CLI.md)) |
| **Translation (real corpus)** | `demo_translate` | S2ELab patterns translated and run on their own examples |
| **Translation (construct breadth)** | `demo_translate_categories` | One example per accepted frontend construct |
| **Translation (rejection causes)** | `demo_translate_reject` | One example per rejected cause |
| **Translation (sizes)** | `demo_translate_sizes` | Node count of the `Regex` each pattern translates to |
| **Dataset pipeline** | `dataset_prepare`, `dataset_stats` | Extract, verify, and report on the raw corpus |
| **Parser agreement** | `demo_parser_agreement`, `demo_parser_agreement_pathological`, `demo_parser_agreement_worker` | Cross-parser agreement on parse tree shape, over the prepared S2ELab dataset and the pathological dataset |
| **Thesis figures** | `demo_simp_branches`, `demo_growth_deriv`, `demo_growth_pderiv`, `demo_growth_eval` | Node growth figures |

Stack-overflow sweeps live in `benches/bench_stack_depth.rs`. See that file's module doc.


## Matching demo

```bash
# All three matchers side by side, no parsing or diagnostics
cargo run --example demo_match
```


## Parsing demo

```bash
# Any one of the five parsers, with optional diagnostics
# --diag 1|2|3 works with a single parser only
# --parser all --diag > 0 errors
# --diag 3 writes reports/diag/<parser>/demo_NN.txt per test case
cargo run --example demo_parse -- --diag 3 --parser deriv_std_rec
cargo run --example demo_parse -- --diag 3 --parser deriv_std_loop
cargo run --example demo_parse -- --diag 3 --parser deriv_bc
cargo run --example demo_parse -- --diag 3 --parser pderiv_std
cargo run --example demo_parse -- --diag 3 --parser pderiv_bc

# All five in one table plus a summary block
cargo run --example demo_parse -- --parser all
```


## Translation demos


### Translate S2-ELAB

```bash
# A size spread of real S2ELab patterns,
# translated and run against their own given match/non-match examples
cargo run --release --example demo_translate -- data/raw/s2-elab/RegexEval.json --diag 1
```

A compact run summary goes to `reports/demos/translate.csv`.

A separate listing covers every pattern this run translates successfully. Each entry shows the original raw pattern text next to its translated `Regex`. Written to `reports/demos/translate_patterns.csv`.


### Rejected patterns

```bash
# Example patterns per rejected cause
cargo run --release --example demo_translate_reject
```

Each example triggers the corresponding `parse_pcre_rule` error.

POSIX classes and an over-`MAX_BOUND` bound each have a named cause, backed by distinct messages in `translate.rs` and `parse.rs`.

Writes `reports/demos/translate_reject.csv`.


### Accepted patterns

```bash
# Example patterns per accepted frontend construct
cargo run --release --example demo_translate_categories
```

Every category here passes, independent of what the S2ELab corpus happens to contain.

Writes a compact per-category summary to `reports/demos/translate_categories.csv`, and the full listing (each category's translated `Regex` plus every check) to `reports/demos/translate_categories_patterns.json`.


### Translation sizes

```bash
# Sizes of the Regex produced by frontend translation,
# wildcard_run() size, case-fold cost
cargo run --release --example demo_translate_sizes
```

Writes `reports/demos/translate_sizes.csv`.

A pattern is either translated as-is (`Direct`), has its redundant top-level `^...$` stripped first (`Explicit full-match`), or rejected outright. See [DATASET.md](DATASET.md) for what each tier means and how `dataset_stats` reports rejection causes over the real corpus.


## Node Growth demos

Each demo writes one or more CSV files under `reports/demos/`, replacing any previous run. Example filenames below.

```bash
# Branch counts, and why simp's r + r = r never fires on (a+(b+ab))*
# -> reports/demos/simp_branches.csv
cargo run --release --example demo_simp_branches

# Brzozowski derivative, expression size per step
# -> reports/demos/growth_deriv.csv
cargo run --release --example demo_growth_deriv

# Antimirov partial derivative, frontier size per input char
# -> reports/demos/growth_pderiv.csv
cargo run --release --example demo_growth_pderiv

# All four growth families side by side
# -> reports/demos/growth_eval_astar.csv, reports/demos/growth_eval_r2.csv
cargo run --release --example demo_growth_eval
```

Growth CSVs use long format, one row per `(series, step)` pair.


## Parser agreement

```bash
# Cross-parser agreement over every prepared case. Run dataset_prepare first.
cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement
```

Saves the summary table to `reports/demos/parser_agreement.csv`, one row per source plus a total. Saves every flagged entry to `reports/demos/parser_agreement_samples.csv`, with a `kind` column covering timeout, crash, membership bug, POSIX shape bug, non-POSIX shape bug, and ambiguous shape.

The pathological counterpart uses `data/pathological/pathological_patterns.json` in place of the S2ELab corpus. Same worker, same three checks (membership, POSIX-internal shape, non-POSIX-internal shape), same POSIX-vs-Non-POSIX shape-agreement column. Rows group by category A to L.

```bash
cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement_pathological
```

Narrowing uses the same `PATHO_*` env vars as `bench_pathological_time`. A large-n sweep needs a correspondingly larger `AGREEMENT_TIMEOUT_MS` (default 2000ms).

```bash
PATHO_CATEGORY=L cargo run --release --example demo_parser_agreement_pathological
PATHO_ROW_ID=P01 PATHO_N=20,50,100 AGREEMENT_TIMEOUT_MS=5000 \
  cargo run --release --example demo_parser_agreement_pathological
```

Saves `reports/demos/parser_agreement_pathological.csv` (summary) and `reports/demos/parser_agreement_pathological_samples.csv` (flagged entries).

Timeouts are expected here. The pathological dataset exists to find each parser's real limit.

