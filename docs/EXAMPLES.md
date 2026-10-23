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
| **Parser agreement** | `demo_parser_agreement`, `demo_parser_agreement_worker` | Cross-parser agreement on parse tree shape over the prepared dataset |
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
Every pattern this run translates successfully is also saved into
`reports/demos/translate.txt`, showing the original raw pattern text
next to its translated `Regex`.


### Rejected Patterns

```bash
# Example patterns per rejected cause
cargo run --release --example demo_translate_reject
```
Each example is verified against the `parse_pcre_rule` error.
POSIX classes and an over-`MAX_BOUND` bound get their own named
cause here, since `translate.rs`/`parse.rs` already produce distinct
messages for both rather than folding them into a catch-all. 
Writes `reports/demos/translate_reject.txt`.


### Accepted Patterns
```bash
# Example patterns per accepted frontend construct
cargo run --release --example demo_translate_categories
```

Every category is expected to pass, unlike `demo_translate` above which
can only show what the given S2ELab file happens to contain. 
Writes `reports/demos/translate_categories.txt`.


### Translation Sizes
```bash
# Sizes of the Regex produced by frontend translation,
# wildcard_run() size, case-fold cost
cargo run --release --example demo_translate_sizes
```

A pattern is either translated as-is (`Direct`), has its redundant
top-level `^...$` stripped first (`Explicit full-match`), or rejected
outright. See [DATASET.md](DATASET.md) for what each tier means and how
`dataset_stats` reports rejection causes over the real corpus.




## Thesis figure demos

Each of these also saves its output to `reports/demos/<name>.txt`
(e.g. `reports/demos/growth_deriv.txt`), replacing any previous run.
All five are deterministic given the algorithms they measure, so a
saved copy only goes stale when those algorithms themselves change.

```bash
# Branch counts, and why simp's r + r = r never fires on (a+(b+ab))*
cargo run --release --example demo_simp_branches

# Brzozowski derivative: expression size per step
cargo run --release --example demo_growth_deriv

# Antimirov partial derivative: frontier size per input char
cargo run --release --example demo_growth_pderiv

# All four growth families side by side
cargo run --release --example demo_growth_eval
```


## Parser Agreement
```bash
# Cross-parser agreement over every prepared case (run dataset_prepare first).
cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement
```

Also saves its output to `reports/demos/parser_agreement.txt`,
replacing any previous run.