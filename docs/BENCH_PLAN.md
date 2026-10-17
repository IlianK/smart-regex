# Bench plan

Prerequisite, once per checkout:

```bash
cargo build --release --example bench_worker
```

`BENCH_SPLIT_OUTCOME=1` is set on every `bench_corpus_time` run below.
It costs nothing -- the match/non-match split is computed in the same
pass regardless of the flag; it only gates whether the second table
gets printed -- so there is no reason to ever omit it.

Every run below also sets `BENCH_RUN_LABEL` to its own T/M/X number,
so each one's CSV lands in its own file (`bench_corpus_time_T3.csv`,
`bench_memory_M3.csv`, `bench_translate_X2_patterns.csv`, ...) instead
of overwriting the last run's. Workflow: name the config here with a
label, run that exact line, look for the labelled file under
`reports/benches/`. One exception: `bench_translate`'s aggregate CSV
(`bench_translate.csv`, never labelled) always appends instead of
writing a separate file per run -- see its own section below.

## Corpus sizes (post-dedup)

`run_pipeline` now skips a rule whose pattern text byte-matches one
already seen earlier in the same `dataset_prepare` run (see
`rules_duplicate_pattern` in `reports/data/dataset_prepare.csv`), so
`patterns_usable` dropped a lot for the two sources that had heavy
internal reuse -- regenerate `prepared.jsonl` for every source before
trusting any run below. Current sizes, N=2 (1 match + 1 non-match)
per pattern:

| Source | Usable patterns | Entries |
|---|---|---|
| Suricata | 2080 | 4160 |
| SpamAssassin | 155 | 310 |
| RegexLib | 2144 | 4288 |
| S2ELab | 438 | 876 |
| Innovatorved | 4795 | 9590 |
| **All 5, combined** | **9612** | **19224** |

Suricata shrank from 6122 (66% was duplicate signatures sharing one
generic detection fragment across many `sid`s); RegexLib from 4099
(48% duplicate, independent users submitting the same validator). The
other three barely moved (SpamAssassin/S2ELab under 5% duplicate;
Innovatorved 4.4%).

The corpus is small enough now, across all five sources combined,
that there is no cost reason left to cap anything or split work across
many separate invocations -- the plan below reflects that: three runs
per target instead of eleven.

## Corpus timing (`bench_corpus_time`)

```bash
# T1 -- smoke: all five sources, minimal cap
BENCH_SOURCE=snort,spamassassin,regexlib,s2-elab,innovatorved \
BENCH_PATTERN_LIMIT=5 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T1 \
  cargo bench --bench bench_corpus_time

# T2 -- default (Suricata + SpamAssassin)
BENCH_RUN_LABEL=T2 cargo bench --bench bench_corpus_time

# T3 -- every source, whole corpus, one run
BENCH_SOURCE=snort,spamassassin,regexlib,s2-elab,innovatorved \
BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T3 \
  cargo bench --bench bench_corpus_time
```

## Corpus memory (`bench_memory`)

No split variable; same sources, same reasoning as the timing plan
above.

```bash
# M1 -- smoke, all five, minimal cap.
BENCH_SOURCE=snort,spamassassin,regexlib,s2-elab,innovatorved \
BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=M1 cargo bench --bench bench_memory

# M2 -- default (Suricata + SpamAssassin)
BENCH_RUN_LABEL=M2 cargo bench --bench bench_memory

# M3 -- every source, whole corpus, one run.
BENCH_SOURCE=snort,spamassassin,regexlib,s2-elab,innovatorved \
BENCH_RUN_LABEL=M3 cargo bench --bench bench_memory
```

When writing up any M run: report `timed_out`/`crashed` alongside `max
peak`, not the peak alone. A `timed_out` entry's worker is killed
before it ever reports, so it contributes nothing to `max peak` --
not a value capped at the timeout, a full exclusion. If any run shows
a nonzero count, the peak you quote is a lower bound on the true
worst case for that source, not the true worst case.

## Translation cost (`bench_translate`)

Frontend translation (`core_regex`: PCRE string -> `Regex`) only --
the step T1-T3/M1-M3 all deliberately exclude from the parser-call
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

Three outputs per run. Two follow the labelled-file convention above;
the aggregate one deliberately doesn't:

- `reports/benches/bench_translate.csv` -- one row per source, always
  this exact name regardless of `BENCH_RUN_LABEL`. Appended to, never
  overwritten, so every run -- one source at a time or all of them in
  one command -- adds its row(s) instead of erasing the last run's. A
  `run_label`/`pattern_limit` column on each row records which
  invocation it came from, since the filename no longer does.
- `..._patterns.csv` -- one row per distinct pattern: nodes/micros/
  bytes plus the translated `Regex` itself, its derived `Debug`
  (`Seq(Lit('a'), ...)`-style -- the six constructors translation
  actually produces, not a flattened reading of them), when small
  enough to be worth a CSV cell (`BENCH_TRANSLATE_RENDER_MAX_NODES`,
  default 150). Still labelled -- kept one catalogue per run, on
  purpose.
- `reports/demos/translation_examples.txt` -- the
  `BENCH_TRANSLATE_EXAMPLES` smallest patterns per source, default 5,
  fully rendered -- a starting point for the "here is what translation
  produces" thesis figure, always overwritten rather than labelled,
  since it is a curated pick, not a measurement run. Hand-pick from
  the full catalogue CSV instead if these aren't the most narratively
  useful rows for a given point.

Translation is cheap enough (no process-spawn overhead, distinct
patterns only) not to need batching for cost reasons -- so keep each
source its own invocation and its own label anyway, same as T/M.
That's what keeps `_patterns.csv` genuinely separate per source, which
is the whole point of that file: a single command covering every
source at once would work too, but collapses all of them into one
shared `_patterns.csv`, which defeats it.

```bash
# X2-X6 -- one per source, whole corpus. 
BENCH_SOURCE=snort BENCH_RUN_LABEL=X2 cargo bench --bench bench_translate
BENCH_SOURCE=spamassassin BENCH_RUN_LABEL=X3 cargo bench --bench bench_translate
BENCH_SOURCE=regexlib BENCH_RUN_LABEL=X4 cargo bench --bench bench_translate
BENCH_SOURCE=s2-elab BENCH_RUN_LABEL=X5 cargo bench --bench bench_translate
BENCH_SOURCE=innovatorved BENCH_RUN_LABEL=X6 cargo bench --bench bench_translate
```

Each run appends its one row to the shared
`reports/benches/bench_translate.csv` overview and writes its own
separate `_X2_patterns.csv`, `_X3_patterns.csv`, etc. Re-running one of
these later (e.g. after a code change) just appends another row for
that source to the overview -- useful for seeing how it moved, not
something to avoid.


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

- **The old per-source, capped plan (T3-T11/M1-M10 in earlier
  revisions of this file) is retired, not just renamed -- it no longer
  appears above.** Those runs measured a corpus with
  65%/48% duplicate-signature weighting baked in for Suricata/RegexLib
  (see "Corpus sizes" above) -- the specific numbers (149.5ms, the
  520x peak gap, the 12244-entry count, all of it) came from a corpus
  that no longer exists in that shape. The *questions* those runs were
  trying to answer mostly still matter; the *answers* need re-deriving
  from T3/M3 against the deduped corpus before any of them go back in
  the thesis. Kept below only as a record of what was asked and why,
  not as current findings.
- **Source selection**: all five in T1-T3/M1-M3 now, no exclusion.
  Innovatorved's old exclusion was about input-generation quality
  (real vs. synthetic match/non-match examples) and about it being the
  corpus that exposed this session's OOM bug -- neither is a reason to
  leave it out of a combined run: its own `timed_out`/`crashed` columns
  in T3/M3's output are exactly where "how often does process
  isolation actually have to intervene" now lives, no separate
  appendix run needed.
- **Caps can mislead -- was the headline finding of the old plan,
  status now open.** T10 (old plan) found `deriv_bc`'s apparent time
  edge over `pderiv` on Suricata reversed between cap 50 and full
  scale, which was real *for that corpus*. Whether the same reversal
  holds on the deduped corpus (T3) is an open question again, not a
  confirmed one -- the "first N patterns in the file" effect the old
  finding described is a different mechanism from duplicate-signature
  weighting, and dedup doesn't obviously fix the former just because it
  fixed the latter. Re-check once T3 lands, don't assume it carries
  over.
- **`pderiv_std` vs `pderiv_bc` on time, match/non-match asymmetry,
  and the M1 5x/520x memory gap** were all old-plan findings with the
  same status: plausible mechanisms (worth re-reading the reasoning
  that produced them), unconfirmed numbers (all computed against the
  pre-dedup corpus). Re-derive from T3/M3, cite those, not the old
  ones.
- **Runtime estimates undercount**, direction known, independent of
  corpus size: `wall (s)` includes a process spawn + pipe round trip
  per entry that `mean` excludes by design, so `wall ÷ entries` and
  `mean` can differ by an order of magnitude on cheap-pattern sources
  (SpamAssassin) and less on expensive ones (Suricata). Doesn't change
  feasibility. `bench_translate`'s own numbers don't have this problem
  -- no process per pattern -- so they're the clean figure for
  translation specifically, not just the cheap one.
- **Isolation safety**: proven on the hardest corpus by content
  (Innovatorved, this session's OOM investigation, ~420MB peak) before
  any of this; T3/M3 re-exercise it at the new combined scale (9612
  patterns across all five sources) but that's a repeat of an
  already-proven mechanism, not a new risk class -- nothing to
  specifically watch for beyond reading `timed_out`/`crashed` off the
  output as usual.
- **Translation cost is measured directly, not just excluded.** X2-X6
  give an actual mean/median/p99/max for `core_regex` itself, by
  source, plus the same four stats for translated size (nodes) --
  worth a sentence correlating cost with size rather than quoting time
  alone; these numbers are already current (collected against the
  deduped corpus) and don't need re-running the way T/M's old numbers
  do. The `_patterns.csv` catalogue and `translation_examples.txt` are
  also where the "here is what translation produces" thesis figure
  should come from -- pick a small pattern, not a real corpus one: a
  character class with an unintended huge range (e.g. `[\--–]`, hyphen
  -to-en-dash) can translate to hundreds of thousands of nodes before
  `core_regex`'s own cap rejects it, so a real corpus pattern's full
  tree is not something to print unconditionally; `bench_translate`
  handles this by only rendering a pattern's tree when it's under
  `BENCH_TRANSLATE_RENDER_MAX_NODES` nodes.
- **Dropped from the old plan, not yet replaced**: T7/T8's
  does-anchoring-change-cost comparison (`BENCH_MATCH_INTENT=direct`
  vs `explicit`). Not part of T1-T3 -- say the word if it's still
  wanted; it would slot in as its own small pair of runs rather than
  folding into the main three.
