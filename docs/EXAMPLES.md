# Examples (Demo)

Runnable demos under `examples/`. These are standalone library-API demos, **not** the `regex-engine` CLI binary -- they read the `REGEX_*` environment variables below directly, independent of the CLI's
`--matcher`/`--parser`/`--diag`/`--diag-report` flags (see [CLI.md](CLI.md)).

---

## Matching demo

```bash
# All three matchers side by side (no parsing / diagnostics)
cargo run --example demo_match
```

## POSIX parsing demo

```bash
# Default (deriv_std_rec, diagnostics off)
cargo run --example demo_posix

# Select parser
REGEX_PARSER=deriv_std_rec  cargo run --example demo_posix
REGEX_PARSER=deriv_std_loop cargo run --example demo_posix
REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# All parsers side by side (comparison table, ignores REGEX_DIAG)
REGEX_PARSER=all cargo run --example demo_posix

# Level 1
REGEX_DIAG=1 cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=deriv_std_loop cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# Level 2
REGEX_DIAG=2 cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=deriv_std_loop cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# Level 3 with report file
REGEX_DIAG=3 cargo run --example demo_posix
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/demo.txt cargo run --example demo_posix
```

## Crash demo (stack overflow: recursive vs. loop)

```bash
# TODO: doc difference for symbols / compiler behaviour
cargo build --example demo_crash_worker
cargo build --example demo_crash_worker --release

cargo run --example demo_crash
cargo run --example demo_crash --release
```

---

## Thesis figure demos

Four small, deterministic programs, one per figure/table in chapters 5-7,
each printing pasteable `pgfplots` coordinates or a plain table directly
to the terminal. None of them write files; the printed numbers are
transcribed into the thesis by hand, not regenerated automatically
(pinned instead by `tests/test_thesis_figures.rs`, so a change in the
implementation fails a named test rather than leaving a figure quietly
wrong). Each was written because the number it reports appears in a
figure or table and had to be reproducible from the repo, per this
project's verification standard: a number written into the thesis with
no runnable source behind it isn't a claim this codebase can back up.

```bash
# Figure 5.3: expression size per derivative step, simplified vs. not.
# Written to make simp's growth-bounding claim checkable, not just argued.
cargo run --release --example demo_growth

# Section 5.5 table: why simp's r + r = r rule never collapses
# (a+(b+ab))*'s branches. Written to show the bit-carrying distinction
# concretely, since "same shape, different bits" is easy to state and
# easy to get wrong without a real trace to check it against.
cargo run --release --example demo_branches

# Figure 6.3: partial-derivative frontier size vs. distinct residual
# count. Written to make the frontier-deduplication argument of
# Section~8.5's "Frontier-wide deduplication" concrete: how much of the
# frontier's growth is genuine duplication vs. genuinely distinct state.
cargo run --release --example demo_frontier

# Figure 7.4: lowered Regex node count per pattern, plus wildcard_run()'s
# own size and case-folding's per-letter cost. Written because Chapter 7's
# claim that a lowered wildcard or search-padding dominates a pattern's
# real cost is a concrete number, not an intuition, once you can print it.
cargo run --release --example demo_lowered
```

---

## Dataset pipeline examples

Two programs that drive `src/data/`'s extract/generate/prepare pipeline
from the command line; see [DATASETS.md](DATASETS.md) for the sources,
the full download instructions, and what each pipeline stage does.

```bash
# Runs extract -> generate -> prepare for one source, writing verified
# Best/Neutral/Worst cases to data/processed/<source>/prepared.jsonl.
cargo run --release --example prepare_dataset -- regexlib data/raw/regexLib/regexlib-manual-processed.txt 

cargo run --release --example prepare_dataset -- snort data/raw/snort/*.rules

cargo run --release --example prepare_dataset -- spamassassin data/raw/spamAssassin/*.cf
```

```bash
# Extracts patterns from raw source files and prints the ones the frontend parser rejects
cargo run --release --example filter_dataset -- regexlib data/raw/regexLib/regexlib-manual-processed.txt 

cargo run --release --example filter_dataset -- snort data/raw/snort/*.rules

cargo run --release --example filter_dataset -- spamassassin data/raw/spamAssassin/*.cf
```
