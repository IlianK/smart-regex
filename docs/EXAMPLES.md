# Examples

Runnable programs under `examples/`, invoked with `cargo run --example <name>`.

| Area | Demos | What they show |
|---|---|---|
| **Matching** | `demo_match` | Naive / Brzozowski / Antimirov matchers side by side |
| **Parsing** | `demo_parse` | Any one of the five parsers, with optional diagnostics.      Reuses the CLI's `--parser`/`--diag`/`--diag-report` names (See [CLI.md](CLI.md)) |
| **Translation — real corpus** | `demo_translate` | S2ELab patterns translated and run on their own examples |
| **Translation — construct breadth** | `demo_translate_categories` | One example per accepted frontend construct |
| **Translation — rejection causes** | `demo_translate_reject` | One example per rejected cause |
| **Translation — sizes** | `demo_translate_sizes` | Node count of the `Regex` each pattern translates to |
| **Dataset pipeline** | `dataset_prepare`, `dataset_stats` | Extract, verify, and report on the raw corpus |
| **Parser agreement** | `demo_parser_agreement`, `demo_parser_agreement_pathological`, `demo_parser_agreement_worker` | Cross-parser agreement on parse tree shape, over the prepared S2ELab dataset and the pathological dataset respectively |
| **Thesis figures** | `demo_simp_branches`, `demo_growth_deriv`, `demo_growth_pderiv`, `demo_growth_eval` | Node growth figures |


## Matching demo

```bash
# All three matchers side by side (no parsing / diagnostics)
cargo run --example demo_match
```


## Parsing demo

```bash
# Any one of the five parsers, with optional diagnostics
# --diag 1|2|3 (single parser only; --parser all --diag > 0 errors)
# --diag 3 writes reports/diag/<parser>/demo_NN.txt per test case
cargo run --example demo_parse -- --diag 3 --parser deriv_std_rec       
cargo run --example demo_parse -- --diag 3 --parser deriv_std_loop
cargo run --example demo_parse -- --diag 3 --parser deriv_bc
cargo run --example demo_parse -- --diag 3 --parser pderiv_std         
cargo run --example demo_parse -- --diag 3 --parser pderiv_bc

# All five in one table plus a summary block
cargo run --example demo_parse -- --parser all
```


Stack-overflow behaviour (recursive `deriv_std_rec` vs. loop `deriv_std_loop`)
moved out of `examples/` into `benches/bench_stack_depth.rs` -- it sweeps
real stress-pattern inputs rather than a single synthetic sample, and reports
a comparable number (largest N survived per parser), which fits this
project's bench-output conventions better than an example's prose. See that
file's own module doc.


## Translation Demos

### Translate S2-ELAB
```bash
# A size spread of real S2ELab patterns,
# translated and run against their own given match/non-match examples
cargo run --release --example demo_translate -- data/raw/s2-elab/RegexEval.json --diag 1
```
A compact run summary is saved into `reports/demos/translate.csv`.
Every pattern this run translates successfully -- not just the few
shown on stdout -- is a separate listing, showing the original raw
pattern text next to its translated `Regex`, saved to
`reports/demos/translate_patterns.csv`.


### Rejected Patterns

```bash
# Example patterns per rejected cause
cargo run --release --example demo_translate_reject
```
Each example is verified against the `parse_pcre_rule` error.
POSIX classes and an over-`MAX_BOUND` bound get their own named
cause here, since `translate.rs`/`parse.rs` already produce distinct
messages for both rather than folding them into a catch-all. 
Writes `reports/demos/translate_reject.csv`.


### Accepted Patterns
```bash
# Example patterns per accepted frontend construct
cargo run --release --example demo_translate_categories
```

Every category is expected to pass, unlike `demo_translate` above which
can only show what the given S2ELab file happens to contain.
Writes a compact per-category summary to
`reports/demos/translate_categories.csv`, and the full listing (each
category's translated `Regex` plus every check) separately to
`reports/demos/translate_categories_patterns.json`.


### Translation Sizes
```bash
# Sizes of the Regex produced by frontend translation,
# wildcard_run() size, case-fold cost
cargo run --release --example demo_translate_sizes
```
Writes `reports/demos/translate_sizes.csv`.

A pattern is either translated as-is (`Direct`), has its redundant
top-level `^...$` stripped first (`Explicit full-match`), or rejected
outright. See [DATASET.md](DATASET.md) for what each tier means and how
`dataset_stats` reports rejection causes over the real corpus.




## Thesis figure demos

Each of these also saves its table(s) as CSV under `reports/demos/`
(e.g. `reports/demos/growth_deriv.csv`, long format: one row per
(series, step)), replacing any previous run. All four are
deterministic given the algorithms they measure, so a saved copy only
goes stale when those algorithms themselves change.

```bash
# Branch counts, and why simp's r + r = r never fires on (a+(b+ab))*
# -> reports/demos/simp_branches.csv
cargo run --release --example demo_simp_branches

# Brzozowski derivative: expression size per step
# -> reports/demos/growth_deriv.csv
cargo run --release --example demo_growth_deriv

# Antimirov partial derivative: frontier size per input char
# -> reports/demos/growth_pderiv.csv
cargo run --release --example demo_growth_pderiv

# All four growth families side by side
# -> reports/demos/growth_eval_astar.csv, reports/demos/growth_eval_r2.csv
cargo run --release --example demo_growth_eval
```


## Parser Agreement
```bash
# Cross-parser agreement over every prepared case (run dataset_prepare first).
cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement
```

Saves the summary table (one row per source plus a total) to
`reports/demos/parser_agreement.csv`, replacing any previous run, and
every individual flagged entry (timeout, crash, membership bug,
POSIX/non-POSIX shape bug, ambiguous shape) separately to
`reports/demos/parser_agreement_samples.csv`.

`demo_parser_agreement`'s counterpart for the pathological dataset
(`data/pathological/pathological_patterns.json`) instead of the real
S2ELab corpus. Same worker (already generic, no S2ELab dependency),
same three checks (membership, POSIX-internal shape, non-POSIX-internal
shape) and the same POSIX-vs-greedy shape-agreement column, grouped by
category (A-L) instead of source:

```bash
cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement_pathological

# Narrow to one category/row, same PATHO_* env vars as bench_pathological_time;
# a large-n sweep needs a correspondingly larger AGREEMENT_TIMEOUT_MS (default 2000ms).
PATHO_CATEGORY=L cargo run --release --example demo_parser_agreement_pathological
PATHO_ROW_ID=P01 PATHO_N=20,50,100 AGREEMENT_TIMEOUT_MS=5000 \
  cargo run --release --example demo_parser_agreement_pathological
```

Saves `reports/demos/parser_agreement_pathological.csv` (summary) and
`reports/demos/parser_agreement_pathological_samples.csv` (flagged
entries), same shape as the S2ELab pair. Timeouts here are expected,
not bugs -- the pathological dataset exists specifically to find each
parser's real limit; a membership/shape bug is still a real bug. One
known false positive: row `E51` (category L) intentionally uses a
non-matching `match_input` by design (see its own `notes` field, and
`bench_stack_depth`, the tool it's actually for) and will always show
as a membership bug here -- that is expected, not a defect.