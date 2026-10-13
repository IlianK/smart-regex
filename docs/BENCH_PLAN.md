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

Every run below also sets `BENCH_RUN_LABEL` to its own T/M number, so
each one's CSV lands in its own file (`bench_corpus_time_T3.csv`,
`bench_memory_M3.csv`, ...) instead of overwriting the last run's.
Workflow: name the config here with a label, run that exact line,
look for the labelled file under `reports/benches/`.

## Corpus timing (`bench_corpus_time`)

```bash
# T1 -- smoke: corpus reads, ballpark.
BENCH_SOURCE=suricata,spamassassin BENCH_PATTERN_LIMIT=5 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T1 \
  cargo bench --bench bench_corpus_time

# T2 -- curated, balanced, whole corpus.
BENCH_SOURCE=s2-elab BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T2 \
  cargo bench --bench bench_corpus_time

# T3 -- main Snort row.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T3 \
  cargo bench --bench bench_corpus_time

# T4 -- main SpamAssassin row, whole corpus.
BENCH_SOURCE=spamassassin BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T4 \
  cargo bench --bench bench_corpus_time

# T5 -- RegexLib row, if in scope.
BENCH_SOURCE=regexlib BENCH_PATTERN_LIMIT=50 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T5 \
  cargo bench --bench bench_corpus_time

# T7 -- does anchoring change cost? Unanchored side.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_MATCH_INTENT=direct BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T7 \
  cargo bench --bench bench_corpus_time

# T8 -- ^...$-recoverable side.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_MATCH_INTENT=explicit BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T8 \
  cargo bench --bench bench_corpus_time

# T9 -- stability check: does the ordering hold at 4x the cap?
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=200 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T9 \
  cargo bench --bench bench_corpus_time

# T10 -- headline whole-corpus number. One-off; wall clock is the
# real unknown here, not safety -- process isolation already proved
# safe on a worse corpus than this (see note below). Not run yet.
BENCH_SOURCE=suricata BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=T10 \
  cargo bench --bench bench_corpus_time
```

## Corpus memory (`bench_memory`)

No split variable; mirrors T3/T4/T5/T2/T9's sources and caps.

```bash
# M1
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=50 BENCH_RUN_LABEL=M1 cargo bench --bench bench_memory

# M2
BENCH_SOURCE=spamassassin BENCH_RUN_LABEL=M2 cargo bench --bench bench_memory

# M3 -- if RegexLib is in scope.
BENCH_SOURCE=regexlib BENCH_PATTERN_LIMIT=50 BENCH_RUN_LABEL=M3 cargo bench --bench bench_memory

# M4
BENCH_SOURCE=s2-elab BENCH_RUN_LABEL=M4 cargo bench --bench bench_memory

# M5 -- stability check, mirrors T9.
BENCH_SOURCE=suricata BENCH_PATTERN_LIMIT=200 BENCH_RUN_LABEL=M5 cargo bench --bench bench_memory
```

When writing up M1-M5: report `timed_out`/`crashed` alongside `max
peak`, not the peak alone. A `timed_out` entry's worker is killed
before it ever reports, so it contributes nothing to `max peak` --
not a value capped at the timeout, a full exclusion. If any run shows
a nonzero count, the peak you quote is a lower bound on the true
worst case for that source, not the true worst case.

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

**Runtime estimates will undercount, direction known.** `wall (s)`
and `mean (ms)` measure different things: `wall` includes one
process spawn and one pipe round trip per entry that `mean`
excludes by design. For Suricata this is a small effect (its
patterns are expensive enough that spawn overhead is a small
fraction of the total); for the three faster parsers it can be a
few-x. Doesn't change feasibility, just don't be surprised if T9/T10
run longer than a mean-based estimate predicts.