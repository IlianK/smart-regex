# Bench plan

Prerequisite, once per checkout:

```bash
cargo build --release --example bench_worker
```

`BENCH_SPLIT_OUTCOME=1` is set on every `bench_corpus_time` run below.
It costs nothing -- the match/non-match split is computed in the same
pass regardless of the flag; it only gates whether the second table
gets printed -- so there is no reason to ever omit it. This also
removes the original plan's T6 (`s2-elab`, uncapped, no split): T2
already is that run, plus the outcome breakdown for free.

Every run below also sets `BENCH_RUN_LABEL` to its own T/M/X number,
so each one's CSV lands in its own file (`bench_corpus_time_T3.csv`,
`bench_memory_M3.csv`, `bench_translate_X2.csv`, ...) instead of
overwriting the last run's. Workflow: name the config here with a
label, run that exact line, look for the labelled file under
`reports/benches/`.

## Corpus timing (`bench_corpus_time`)

```bash
# T1 -- smoke: corpus reads, ballpark.
BENCH_SOURCE=suricata,spamassassin BENCH_PATTERN_LIMIT=5 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T1 \
  cargo bench --bench bench_corpus_time

# T2 -- curated, balanced, whole corpus.
BENCH_SOURCE=s2-elab BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T2 \
  cargo bench --bench bench_corpus_time

# T3 -- Snort at cap 50. Superseded by T10 as the headline number --
# kept as the cap-sensitivity exhibit (see Notes).
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T3 \
  cargo bench --bench bench_corpus_time

# T4 -- main SpamAssassin row, whole corpus.
BENCH_SOURCE=spamassassin BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T4 \
  cargo bench --bench bench_corpus_time

# T5 -- RegexLib row at cap 50. Superseded by T11 -- see Notes.
BENCH_SOURCE=regexlib BENCH_PATTERN_LIMIT=50 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T5 \
  cargo bench --bench bench_corpus_time

# T7 -- does anchoring change cost? Unanchored side.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_MATCH_INTENT=direct BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T7 \
  cargo bench --bench bench_corpus_time

# T8 -- ^...$-recoverable side.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_MATCH_INTENT=explicit BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T8 \
  cargo bench --bench bench_corpus_time

# T9 -- stability check at 4x T3's cap. Also superseded by T10 as a
# headline number, kept for the same reason as T3.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=200 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T9 \
  cargo bench --bench bench_corpus_time

# T10 -- whole Snort corpus (12244 entries). The headline timing
# number; see Notes for what running it changed.
BENCH_SOURCE=suricata BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T10 \
  cargo bench --bench bench_corpus_time

# T11 -- whole RegexLib corpus. T5's cap=50 anomalies (pderiv-only
# timeouts, pderiv memory exceeding deriv_std) have the same
# "artifact or real" question T10 just answered for Suricata.
BENCH_SOURCE=regexlib BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T11 \
  cargo bench --bench bench_corpus_time
```

## Corpus memory (`bench_memory`)

No split variable; mirrors the timing sources/caps above.

```bash
# M1 -- Suricata at cap 50. Superseded by M10 -- see Notes.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_RUN_LABEL=M1 cargo bench --bench bench_memory

# M2
BENCH_SOURCE=spamassassin BENCH_RUN_LABEL=M2 cargo bench --bench bench_memory

# M3 -- RegexLib at cap 50. Superseded by M6.
BENCH_SOURCE=regexlib BENCH_PATTERN_LIMIT=50 BENCH_RUN_LABEL=M3 cargo bench --bench bench_memory

# M4
BENCH_SOURCE=s2-elab BENCH_RUN_LABEL=M4 cargo bench --bench bench_memory

# M5 -- stability check, mirrors T9. Superseded by M10.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=200 BENCH_RUN_LABEL=M5 cargo bench --bench bench_memory

# M6 -- whole RegexLib corpus. Pairs with T11.
BENCH_SOURCE=regexlib BENCH_RUN_LABEL=M6 cargo bench --bench bench_memory

# M10 -- whole Suricata corpus. Pairs with T10, and is now the single
# highest-priority run left: the chapter's strongest claim (M1's ~5x
# total / ~520x peak gap) is still resting on a cap=50 sample, the
# same unverified-at-scale status T3's timing number had before T10.
BENCH_SOURCE=suricata BENCH_RUN_LABEL=M10 cargo bench --bench bench_memory
```

When writing up any M run: report `timed_out`/`crashed` alongside `max
peak`, not the peak alone. A `timed_out` entry's worker is killed
before it ever reports, so it contributes nothing to `max peak` --
not a value capped at the timeout, a full exclusion. If any run shows
a nonzero count, the peak you quote is a lower bound on the true
worst case for that source, not the true worst case.

## Translation cost (`bench_translate`)

Frontend translation (`core_regex`: PCRE string -> `Regex`) only --
the step T1-T11/M1-M10 all deliberately exclude from the parser-call
numbers above (see `bench_worker.rs`'s module doc). Measured by
*distinct pattern*, not by entry: translation cost depends only on the
pattern, so timing it per entry (the way match cost is timed) would
re-measure the same pattern's translation once per input it carries
and report that as independent samples. No worker-process isolation
either, unlike every other bench here -- every pattern already passed
`core_regex` once during `dataset_prepare`, so re-running it is a
repeat of an already-proven-safe call, and `core_regex` enforces its
own depth/node caps before the expensive part of translation runs.
See `bench_translate.rs`'s module doc for the full argument. One
consequence: no process-spawn overhead, so this file's timing numbers
are cleaner than `wall (s)` above, not just cheaper to produce.

Three outputs per run: the aggregate CSV below (mean/median/p99/max of
time, allocation, and translated size, per source); a full per-pattern
catalogue (`..._patterns.csv`, one row per distinct pattern, including
the translated `Regex` itself -- its derived `Debug`, `Seq(Lit('a'),
...)`-style, showing the six constructors translation actually
produces rather than a flattened reading of them -- when small enough
to be worth a CSV cell (`BENCH_TRANSLATE_RENDER_MAX_NODES`, default
150)); and `reports/demos/translation_examples.txt` (the
`BENCH_TRANSLATE_EXAMPLES` smallest patterns per source, default 5,
fully rendered -- a starting point for the "here is what translation
produces" thesis figure, always overwritten rather than labelled,
since it is a curated pick, not a measurement run. Hand-pick from the
full catalogue CSV instead if these aren't the most narratively useful
rows for a given point).

```bash
# X1 -- smoke.
BENCH_SOURCE=suricata,spamassassin BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=X1 \
  cargo bench --bench bench_translate

# X2-X5 -- one per main-table source, whole corpus.
BENCH_SOURCE=snort BENCH_RUN_LABEL=X2 cargo bench --bench bench_translate
BENCH_SOURCE=spamassassin BENCH_RUN_LABEL=X3 cargo bench --bench bench_translate
BENCH_SOURCE=regexlib BENCH_RUN_LABEL=X4 cargo bench --bench bench_translate
BENCH_SOURCE=s2-elab BENCH_RUN_LABEL=X5 cargo bench --bench bench_translate
BENCH_SOURCE=innovatorved BENCH_RUN_LABEL=X5 cargo bench --bench bench_translate
```

Cheap relative to T/M: distinct patterns only, no process-spawn
overhead -- X2-X5 together should take well under a minute even at
Suricata's full scale.


## Synthetic (`bench_parse`, `bench_match`)

No `BENCH_RUN_LABEL` here -- these two are still plain Criterion
targets with their own HTML report per run under `target/criterion/`,
unaffected by anything in this plan.

```bash
# Table-sized -- fast enough to rerun while drafting.
BENCH_SIZE_LIMIT=200 BENCH_SAMPLE_SIZE=30 cargo bench --bench bench_parse
BENCH_SIZE_LIMIT=100 BENCH_SAMPLE_SIZE=30 cargo bench --bench bench_match

# Full sweeps, for the appendix -- back the claim that the capped
# curves above are representative.
cargo bench --bench bench_parse
cargo bench --bench bench_match
```

## Notes

- **Source selection.** Suricata, SpamAssassin, S2ELab, RegexLib in
  the main tables now that T11/M6 exist; justify RegexLib on
  crowd-sourced-but-cheap-to-check grounds, not "maintained production
  ruleset." Innovatorved stays out of the main tables -- not only for
  data-quality reasons, but because it's the corpus that exposed this
  session's OOM bug and verified the fix; see the appendix runs above
  instead.
- **Caps can mislead** T10 found
  `deriv_bc`'s apparent time edge over both `pderiv` variants on
  Suricata (real at cap 50 and cap 200) disappears at full scale:
  mean dropped 149.5ms -> 50.8ms -> 7.06ms across the three caps,
  meaning the first 50-200 patterns in the file are unusually
  expensive, not representative. T3/T9 are kept as the exhibit for
  this, not as headline numbers. The same question is now open for
  RegexLib (T11/M6) and for Suricata's memory story (M10).
- **`pderiv_std` beating `pderiv_bc` on time looks real.** Two large,
  unconfounded comparisons agree: RegexLib (T5, identical entry/timeout
  counts both sides) and full Suricata (T10, only a 2-entry timeout
  difference out of 12244 -- too small to explain the gap). Worth a
  sentence distinguishing this from the cap=200 (M5) case, where
  `pderiv_std` excluded twice as many entries as `pderiv_bc` and the
  apparent edge is at least partly that artifact.
- **Match/non-match asymmetry shrinks at scale.** Real for every
  parser at cap 50; at full Suricata scale (T10) it washes out for
  everyone except `deriv_std_rec`/`loop`, which keep ~13%.
- **Runtime estimates undercount**, direction known: `wall (s)`
  includes a process spawn + pipe round trip per entry that `mean`
  excludes by design, so `wall ÷ entries` and `mean` can differ by an
  order of magnitude on cheap-pattern sources (SpamAssassin) and less
  on expensive ones (Suricata). Doesn't change feasibility.
  `bench_translate`'s own numbers (X-series) don't have this problem --
  no process per pattern -- so they're the clean figure for translation
  specifically, not just the cheap one.
- **Isolation safety is proven at both ends now**: the worst single
  corpus (Innovatorved, this session's OOM investigation, ~420MB peak)
  and the largest one (T10, 12244 entries, 23 timeouts handled
  cleanly). M10/M6 use the identical mechanism; expected safe by the
  same proof, not independently re-verified at this exact scale for
  `bench_memory` specifically.
- **Translation cost is now measured directly, not just excluded.**
  X2-X5 give an actual mean/median/p99/max for `core_regex` itself, by
  source, plus the same four stats for translated size (nodes) --
  worth a sentence correlating cost with size rather than quoting time
  alone. The `_patterns.csv` catalogue and `translation_examples.txt`
  are also where the "here is what translation produces" thesis figure
  should come from -- pick a small pattern, not a real corpus one: a
  character class with an unintended huge range (e.g. `[\--–]`, hyphen
  -to-en-dash) can translate to hundreds of thousands of nodes before
  `core_regex`'s own cap rejects it, so a real corpus pattern's full
  tree is not something to print unconditionally; `bench_translate`
  handles this by only rendering a pattern's tree when it's under
  `BENCH_TRANSLATE_RENDER_MAX_NODES` nodes.
- **If T11/M6 confirm RegexLib's anomaly survives at full scale**,
  worth a small diagnostic naming the specific pattern(s) responsible
  -- a concrete named example reads better than an aggregate anomaly.
  Not built; flag if wanted once T11 lands.
