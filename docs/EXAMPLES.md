# Examples

Runnable programs under `examples/`, invoked with `cargo run --example <name>`.
Self-contained programs demonstrating matching and parsing on the worked examples
(with and without diagnostics), search padding on a sample of the real corpus
(`data/raw/_Samples`), frontend coverage (showing which PCRE features are
supported, approximated, or rejected), stack-depth limits comparing the loop
and recursive approaches for the derivative-based parsers only, 
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


## Search-padding demos (anchored / unanchored samples)

```bash
# Unanchored: noisy input matches the padded regex, not the plain one
cargo run --release --example demo_samples_unanchored -- snort \
  --data-dir data/raw/_Samples --seed 1 --count 3

# Anchored: same, noise only on the padded side(s)
cargo run --release --example demo_samples_anchored -- regexlib \
  --data-dir data/raw/_Samples --seed 1 --count 3
```


## Frontend coverage demo (supported categories)

```bash
# One real corpus pattern per verified frontend behaviour on _Samples
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

# Sizes of the Regex produced by frontend translation, wildcard_run() size, case-fold cost
cargo run --release --example demo_frontend_translation_sizes
```


## Dataset pipeline examples

```bash
# extract -> generate -> prepare; writes data/processed/<source>/prepared.jsonl
cargo run --release --example dataset_prepare -- regexlib data/raw/regexlib/*.txt
cargo run --release --example dataset_prepare -- snort data/raw/snort/*.rules
cargo run --release --example dataset_prepare -- spamassassin data/raw/spamAssassin/*.cf

# --variants controls verified inputs per pattern per category (default 5)
cargo run --release --example dataset_prepare -- regexlib data/raw/regexlib/*.txt --variants 3
```

```bash
# Coverage report: extraction counts, rejection causes, faithful/approx split
cargo run --release --example dataset_stats
cargo run --release --example dataset_stats -- --verbose
```

```bash
# Cross-parser agreement over every prepared case (run dataset_prepare first)
cargo run --release --example demo_parser_agreement
```