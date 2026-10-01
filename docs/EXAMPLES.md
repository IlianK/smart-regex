# Examples (Demo)

Runnable demos under `examples/`. These are standalone library-API demos, **not** the `regex-engine` CLI binary. They read the `REGEX_*` environment variables below directly, independent of the CLI's
`--matcher`/`--parser`/`--diag`/`--diag-report` flags (see [CLI.md](CLI.md)).

---

## Matching demo

```bash
# All three matchers side by side (no parsing / diagnostics)
cargo run --example demo_match
```

## POSIX parsing demo

```bash
# Default (recursive parser, diagnostics off)
cargo run --example demo_posix

# Select parser
REGEX_PARSER=deriv_loop cargo run --example demo_posix
REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# All parsers side by side (comparison table, ignores REGEX_DIAG)
REGEX_PARSER=all cargo run --example demo_posix

# Level 1
REGEX_DIAG=1 cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=deriv_loop cargo run --example demo_posix
REGEX_DIAG=1 REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# Level 2
REGEX_DIAG=2 cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=deriv_loop cargo run --example demo_posix
REGEX_DIAG=2 REGEX_PARSER=deriv_bc   cargo run --example demo_posix

# Level 3 with report file
REGEX_DIAG=3 cargo run --example demo_posix
REGEX_DIAG=3 REGEX_DIAG_REPORT=reports/demo.txt cargo run --example demo_posix
```

## Crash demo (stack overflow: recursive vs. loop)

```bash
cargo build --example demo_crash_worker
cargo build --example demo_crash_worker --release

cargo run --example demo_crash
cargo run --example demo_crash --release
```

`demo_crash` spawns `demo_crash_worker` as a subprocess and binary-searches
for the input length at which each parser overflows its stack, so a crash
in the worker is observed as a non-zero exit status rather than taking the
driver down with it. The absolute limits differ between a debug and a
release build (the driver prints the mode it is running in); compare the
two builds of the *same* parser, not the same build of two parsers, when
quoting a number.

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
# Also prints the two prose claims from Section 5.5 (sizes 2303 and 36863
# at steps 16 and 24) and re-checks the "doubles every two characters"
# claim directly, so the prose numbers and the figure come from one run.
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

## Anchor / padding demos

Two demos that make `frontend::translate_as_search`'s padding rule
visible on real corpus patterns: what it buys for an unanchored pattern
(Sigma* on both sides = substring search), and what it looks like for a
pattern that carries `^` and/or `$` (padding on only the uncovered
side(s), nothing at all for a fully-anchored `^...$`). Both draw from the
same sample files under `--data-dir`, share the same argument surface, and
can be pointed at either pool with the same command line.

Both use the project's own dataset pipeline end to end (`data::extract`
to load, `data::prepare::core_regex` to translate, `data::generate` for
candidate inputs, `frontend::parse_pcre_rule` for the padded regex), so
what they print is what the real benchmark is built from, not a
hand-written example.

```bash
# Unanchored patterns: generate a minimal match, wrap it in noise on both
# sides, then show it MATCHES the search-padded regex and does NOT match
# the same body with no padding -- the same input string, differing only
# in whether Sigma* is present.
cargo run --release --example demo_unanchored_samples -- regexlib \
  --data-dir data/raw/_Samples --seed 1 --count 3

cargo run --release --example demo_unanchored_samples -- snort \
  --data-dir data/raw/_Samples --seed 1 --count 3

# Anchored patterns: same idea, but noise is added only on the side(s)
# the padding actually covers. A fully-anchored ^...$ sample has no
# padding to exercise and is traced against the padded regex only.
cargo run --release --example demo_anchored_samples -- regexlib \
  --data-dir data/raw/_Samples --seed 1 --count 3

cargo run --release --example demo_anchored_samples -- snort \
  --data-dir data/raw/_Samples --seed 1 --count 3
```

Both accept `--seed N` (deterministic sample selection and input
generation), `--data-dir DIR` (default `data/_Samples` for the anchored
demo, `data/raw/_Samples` for the unanchored one), `--count N` (default
3), `--diag 0|1|2|3` (default 1), and explicit file paths to override the
per-source sample-file list. `--diag 3` will write a report file per
test case, same as `demo_posix`.

---

## Dataset pipeline examples

Programs that drive `src/data/`'s extract/generate/prepare pipeline from
the command line; see [DATASETS.md](DATASETS.md) for the sources, the
full download instructions, and what each pipeline stage does.

```bash
# Runs extract -> generate -> prepare for one source, writing verified
# Best/Neutral/Worst cases to data/processed/<source>/prepared.jsonl.
cargo run --release --example prepare_dataset -- regexlib data/raw/regexLib/regexlib-manual-processed.txt

cargo run --release --example prepare_dataset -- snort data/raw/snort/*.rules

cargo run --release --example prepare_dataset -- spamassassin data/raw/spamAssassin/*.cf
```

`--variants N` (default 5) is how many independently verified inputs each
pattern contributes per category, not a total across categories: up to N
`Best`, N `Neutral`, and 2N `Worst` (N heavily-iterated positives, N
late-failing negatives) survive verification per pattern.

```bash
# Extracts patterns from raw source files and prints the ones the frontend parser rejects
cargo run --release --example filter_dataset
```

`filter_dataset` reads all three sources from their standard locations
under `data/raw/` by default. `--source S` restricts to one, `--verbose`
lists each rejected pattern with its cause, and `--no-anchors` /
`--no-flags` / `--no-caveats` suppress the corresponding tables. The
"Faithful" row it prints is not just a statistic about the corpus: it is
exactly the set of patterns `prepare_dataset` builds the benchmark from,
so a difference between the two is a bug in one of them, not a
difference of opinion.

```bash
# Finds, per category of frontend behaviour this project has verified,
# a real pattern from the given dataset files that exercises it, and
# shows the frontend's actual behaviour on that exact pattern.
cargo run --release --example demo_frontend_coverage -- \
  data/raw/_Samples/emerging-exploit.rules data/raw/_Samples/emerging-web_client.rules \
  data/raw/_Samples/20_body_tests.cf data/raw/_Samples/20_drugs.cf data/raw/_Samples/20_head_tests.cf \
  data/raw/_Samples/regexlib-manual-processed.sample.txt --seed 1 --diag 1
```

`demo_frontend_coverage` has no fallback file list: it exits if no files
are given on the command line, and each category whose example is not
found in the given files is reported as skipped rather than filled in
with a synthetic pattern. `--seed N` makes candidate generation
deterministic; `--diag 0|1|2|3` sets trace verbosity (default 1).

```bash
# Cross-parser agreement over the prepared dataset: for every
# (pattern, input) entry, runs all five parsers and checks that the
# POSIX family (rec/loop/bc) and the Greedy family (pstd/pbc) are each
# internally shape-consistent, that membership agrees across families,
# and that all five agree with what prepare::verify_candidate recorded
# when the dataset was built.
cargo run --release --example parser_agreement
```

Reads `data/processed/`, so run `prepare_dataset` first. Narrows the same
way the criterion benches do via `BENCH_CATEGORY`, `BENCH_SOURCE`, and
`BENCH_PATTERN_LIMIT` (see [BENCHMARKS.md](BENCHMARKS.md)); unlike the
benches, `BENCH_PATTERN_LIMIT` is unset by default here, so a bare run
checks every prepared entry. `AGREEMENT_TIMEOUT_MS` (default 2000) bounds
each parser call; a call that does not return in time is tallied as a
timeout rather than a bug, and its thread is abandoned (Rust cannot force
a thread to stop), so a run with many timeouts will keep consuming CPU
and memory until the process exits. `deriv_std_rec` / `deriv_std_loop`
run the *unsimplified* derivative, and every corpus pattern is
search-padded before this tool sees it, so timeouts on large patterns are
expected and are not a defect in the tool.