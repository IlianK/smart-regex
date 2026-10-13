# Examples

Runnable programs under `examples/`, invoked with `cargo run --example <name>`.
Self-contained programs demonstrating matching and parsing on the worked examples
(with and without diagnostics), reject-cause coverage on a sample of the real
corpus (`data/raw/_Samples`), frontend coverage (showing which PCRE features are
supported or rejected), stack-depth limits comparing the loop and recursive
approaches for the derivative-based parsers only, the dataset pipeline itself,
and demos that reproduce the numbers behind the thesis figures.

`demo_parse` reuses the CLI's `--parser`/`--diag`/`--diag-report` names.
For the CLI itself, see [CLI.md](CLI.md).


## Matching demo

```bash
# All three matchers side by side (no parsing / diagnostics)
cargo run --example demo_match
```


## Parsing demo

```bash
# Any one of the five parsers, with optional diagnostics
cargo run --example demo_parse -- --parser deriv_std_rec       # default
cargo run --example demo_parse -- --parser deriv_std_loop
cargo run --example demo_parse -- --parser deriv_bc
cargo run --example demo_parse -- --parser pderiv_std          # Greedy, not POSIX
cargo run --example demo_parse -- --parser pderiv_bc

# All five in one table plus a summary block
cargo run --example demo_parse -- --parser all

# --diag 1|2|3 (single parser only; --parser all --diag > 0 errors)
# --diag 3 writes reports/<parser>/demo_NN.txt per test case
cargo run --example demo_parse -- --diag 3 --parser deriv_bc

# Individual path
cargo run --example demo_parse -- --diag 3 --parser deriv_bc --diag-report reports/own_name.txt
```


## Crash demo (stack overflow: recursive vs. loop)

```bash
# demo_crash spawns demo_crash_worker; build the worker first
cargo build --example demo_crash_worker
cargo build --example demo_crash_worker --release

cargo run --example demo_crash
cargo run --example demo_crash --release
```


## Reject-cause demo

```bash
# One real corpus pattern per rejection cause dataset_stats.rs counts
# (backreference, lookaround, word-boundary, anchor, flag-m, flag-x, other),
# read from the standard raw directories (data/raw/{snort,spamAssassin,regexLib})
cargo run --release --example demo_canonical_reject

# Or against an explicit file list instead of the standard directories
cargo run --release --example demo_canonical_reject -- data/raw/_Samples/emerging-exploit.rules
```

A pattern is either translated as-is (`Direct`), has its redundant
top-level `^...$` stripped first (`Explicit full-match`), or rejected
outright. See [DATASET.md](DATASET.md) for what each tier means and how
`dataset_stats` reports them.


## Frontend coverage demo (supported categories)

```bash
# One real corpus pattern per verified frontend behaviour 
cargo run --release --example demo_frontend_categories -- \
  data/raw/_Samples/emerging-exploit.rules data/raw/_Samples/emerging-web_client.rules \
  data/raw/_Samples/20_body_tests.cf data/raw/_Samples/20_drugs.cf data/raw/_Samples/20_head_tests.cf \
  data/raw/_Samples/regexlib-manual-processed.sample.txt --seed 1 --diag 1
```


## Thesis figure demos

```bash
# Branch counts, and why simp's r + r = r never fires on (a+(b+ab))*
cargo run --release --example demo_simp_branches

# Brzozowski derivative: expression size per step
cargo run --release --example demo_growth_deriv

# Antimirov partial derivative: frontier size per input char
cargo run --release --example demo_growth_pderiv

# All four growth families side by side
cargo run --release --example demo_growth_eval

# Sizes of the Regex produced by frontend translation, wildcard_run() size, case-fold cost
cargo run --release --example demo_frontend_translation_sizes
```


## Dataset pipeline examples

Default focus is Suricata (`snort`) and SpamAssassin: both are real-world
IDS/mail-filter rule sets with a maintained upstream. RegexLib is
user-submitted with no correctness guarantee, so it is opt-in only -- pass
it explicitly if you want it.

```bash
# Coverage report: extraction counts, accepted/rejected split, 
# rejection causes, and anchor-shape distribution. 
# No args needed: reads data/raw/{snort,spamAssassin}  

cargo run --release --example dataset_stats
cargo run --release --example dataset_stats -- --verbose
cargo run --release --example dataset_stats -- --source regexlib
```

```bash
# extract -> generate -> prepare; writes data/processed/<source>/prepared.jsonl
cargo run --release --example dataset_prepare -- suricata data/raw/snort/*.rules
cargo run --release --example dataset_prepare -- spamassassin data/raw/spamAssassin/*.cf

# opt-in, not run by default (see above)
cargo run --release --example dataset_prepare -- regexlib data/raw/regexLib/*.txt

# --variants controls verified inputs per pattern per category (default 5)
cargo run --release --example dataset_prepare -- suricata data/raw/snort/*.rules --variants 3
```

```bash
# Cross-parser agreement over every prepared case (run dataset_prepare first).
cargo build --release --example demo_parser_agreement_worker
cargo run --release --example demo_parser_agreement

# BENCH_SOURCE (comma-separated suricata/snort/spamassassin/regexlib),
# BENCH_PATTERN_LIMIT (distinct patterns per source), and
# AGREEMENT_TIMEOUT_MS (per-call kill timeout) narrow or retune a run:
BENCH_SOURCE=spamassassin BENCH_PATTERN_LIMIT=10 cargo run --release --example demo_parser_agreement
```
