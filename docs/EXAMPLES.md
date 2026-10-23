# Examples

Runnable programs under `examples/`, invoked with `cargo run --example <name>`.
Self-contained programs demonstrating matching and parsing on the worked examples
(with and without diagnostics), frontend/translation coverage split across three
demos (real S2ELab patterns in `demo_translate`, synthetic construct breadth in
`demo_translate_categories`, hand-authored rejection causes in
`demo_translate_reject`), stack-depth limits comparing the loop and recursive
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
# One hand-authored pattern per frontend rejection cause -- backreference,
# lookaround, word-boundary, flag-m, flag-x, POSIX class, bound-too-large,
# anchor. Synthetic and self-contained: no corpus file, no CLI args.
cargo run --release --example demo_translate_reject
```

Each example is verified against the real `parse_pcre_rule` error, not
assumed; POSIX classes and an over-`MAX_BOUND` bound get their own named
cause here, since `translate.rs`/`parse.rs` already produce distinct
messages for both rather than folding them into a catch-all. Writes
`reports/demos/translate_reject.txt`.

A pattern is either translated as-is (`Direct`), has its redundant
top-level `^...$` stripped first (`Explicit full-match`), or rejected
outright. See [DATASET.md](DATASET.md) for what each tier means and how
`dataset_stats` reports rejection causes over the real corpus; its
`Cause::Other` catch-all is untouched by this demo's extra two causes,
which are specific to this demo, not a change to `dataset_stats` itself.


## Translation demo (real S2ELab patterns)

```bash
# A size spread of real S2ELab patterns, translated and run against
# their own given match/non-match examples
cargo run --release --example demo_translate -- data/raw/s2-elab/RegexEval.json --diag 1
```

Every pattern this run translates successfully -- not just the few
shown on stdout -- is also dumped to `reports/demos/translate.txt`, raw
pattern text next to its translated `Regex`.


## Translation breadth demo (synthetic categories)

```bash
# One hand-authored pattern per accepted frontend construct: literal,
# '.' with/without '\n', each escape class, classes, bounded quantifiers,
# alternation, quantifiers, groups (plain/non-capturing/named),
# case-folding, lazy-marker discard, 'ε'/'∅'. No corpus, no CLI args.
cargo run --release --example demo_translate_categories
```

Every category is expected to pass -- a failure here is a bug in the
example itself, not a dataset gap, unlike `demo_translate` above which
can only show what the given S2ELab file happens to contain. Writes
`reports/demos/translate_categories.txt`.


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

# Sizes of the Regex produced by frontend translation, wildcard_run() size, case-fold cost
cargo run --release --example demo_translate_sizes
```


## Parser Agreement
```bash
# Cross-parser agreement over every prepared case (run dataset_prepare first).
cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement
```

Also saves its output to `reports/demos/parser_agreement.txt`,
replacing any previous run -- the one demo in this file whose exit
code matters (1 on a real bug) and whose "last known clean" state is
worth a persisted record rather than only ever existing in scrollback.