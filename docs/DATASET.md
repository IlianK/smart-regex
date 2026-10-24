# Datasets

Two pattern sources:

- **S2ELab** -- real human-authored regex benchmark with its own given
  test inputs.
- **Pathological stress patterns** -- hand-curated patterns with
  generated inputs, no author-given examples (see
  [Pathological stress patterns](#pathological-stress-patterns) below).

Suricata/Snort, SpamAssassin, RegexLib and Innovatorved are still read
by `dataset_stats`'s raw-corpus diagnostics for historical comparison,
but no longer feed `dataset_prepare`/`run_pipeline`, and aren't used by
any benchmark.

## S2ELab

762 patterns, each with a natural-language description and the
author's own labeled examples. Lives at
`data/raw/s2-elab/RegexEval.json`, taken from
[github.com/s2e-lab/RegexEval](https://github.com/s2e-lab/RegexEval).

One entry (`id: 100`), fields trimmed:

```json
{
  "expression": "^(0[1-9]|1[0-2])\/((0[1-9]|2\\d)|3[0-1])\/(19\\d\\d|200[0-3])$",
  "matches": ["12/31/2003", "01/01/1900", "11/31/2002", "10/31/2003", "..."],
  "non_matches": ["1/1/2002", "01/01/02", "01/01/2004", "12/123/4322", "..."]
}
```

Real patterns with real author-given labels, sometimes debatable but
always what a human intended. `run_pipeline` trusts each entry's own
`matches`/`non_matches` wherever they agree with the real translated
`Regex`; nothing else about S2ELab's labels is re-checked.

## Dataset statistics

```bash
cargo run --release --example dataset_stats                              # all default sources
cargo run --release --example dataset_stats -- --verbose --source s2-elab # list rejects + cause
cargo run --release --example dataset_stats -- --source suricata --source s2-elab
```

Flags: `--source S` restrict to one source, `--verbose` list each
rejected pattern next to its cause, `--no-flags` suppress the two flag
tables (and CSVs). Below is a run of all 5 sources.

**Dataset coverage** -- *Accepted*/*Rejected* count distinct raw
pattern text, not rule instances. *Dup* is how many extracted
instances dedup removed. *Inputs Provided* is S2ELab's own given
`matches`+`non_matches`; 0 for every other source.

| Source | Files | Extracted | Inputs Provided | Dup | Accepted | Acc% | Rejected | Rej% |
|---|---|---|---|---|---|---|---|---|
| snort | 47 | 16774 | 0 | 6042 | 2082 | 34.0 | 8650 | 81.2 |
| spamassassin | 62 | 549 | 0 | 13 | 155 | 95.7 | 381 | 98.4 |
| regexlib | 2 | 6506 | 0 | 3055 | 2172 | 52.0 | 1279 | 54.8 |
| s2-elab | 1 | 762 | 18930 | 7 | 578 | 98.8 | 177 | 100.0 |
| innovatorved | 1 | 8549 | 0 | 340 | 4803 | 95.6 | 3406 | 96.6 |
| **total** | **113** | **33140** | **18930** | **10186** | **9230** | **57.4** | **13724** | **80.4** |

**Accepted by match-intent reliability** -- `Direct`/`UnanchoredAmbiguous`
= no anchor, translated as-is; `Explicit ^...$`/`ExplicitFullMatch` =
top-level wrap, anchors stripped (any other placement is rejected).

| Source | UnanchoredAmbiguous | % | ExplicitFullMatch `^...$` | % |
|---|---|---|---|---|
| snort | 1623 | 78.0 | 459 | 22.0 |
| spamassassin | 132 | 85.2 | 23 | 14.8 |
| regexlib | 881 | 40.6 | 1291 | 59.4 |
| s2-elab | 115 | 19.9 | 463 | 80.1 |
| innovatorved | 3358 | 69.9 | 1445 | 30.1 |
| **total** | **6000** | **65.0** | **3230** | **35.0** |

**Rejection reasons** (one column per cause `core_regex_tagged` gave):

| Source | backreference | lookaround | word-boundary | anchor | flag-m | flag-x | other |
|---|---|---|---|---|---|---|---|
| snort | 215 | 98 | 19 | 6264 | 3963 | 0 | 92 |
| spamassassin | 8 | 50 | 164 | 142 | 19 | 0 | 4 |
| regexlib | 69 | 594 | 153 | 901 | 9 | 0 | 607 |
| s2-elab | 3 | 43 | 23 | 106 | 0 | 0 | 2 |
| innovatorved | 154 | 1324 | 239 | 1052 | 2 | 0 | 754 |
| **total** | **449** | **2109** | **598** | **8465** | **3993** | **0** | **1459** |

**Anchor shape** (every extracted pattern, accepted or not) --
*Unparseable*: rejected before an `ExtPat` even exists. *Fully-wrapped*:
recovered by `core_regex_tagged`. *start/end/nested*: rejected.

| Source | unparseable | none | fully-wrapped | start-only | end-only | nested |
|---|---|---|---|---|---|---|
| snort | 372 | 5739 | 2783 | 5934 | 1669 | 277 |
| spamassassin | 63 | 304 | 34 | 136 | 8 | 4 |
| regexlib | 1232 | 1766 | 2622 | 197 | 98 | 591 |
| s2-elab | 48 | 138 | 472 | 16 | 8 | 80 |
| innovatorved | 2170 | 3745 | 1580 | 445 | 195 | 414 |
| **total** | **3885** | **11692** | **7491** | **6728** | **1978** | **1366** |

## Dataset preparation

No input is generated, derived, or heuristically assembled.
`run_pipeline` is **S2ELab-only**: extracts and translates patterns,
then writes each usable pattern's own `matches`/`non_matches`, each
re-checked against the real translated `Regex` via `matches_bounded`
(a disagreement is dropped, `inputs_label_mismatch`). The other four
sources are still read by `dataset_stats`, never by `run_pipeline`.

```bash
cargo run --release --example dataset_prepare -- data/raw/s2-elab/RegexEval.json
```

Writes `data/processed/s2-elab/prepared.jsonl`, replacing the file.

**Every usable pattern contributes the same total row count**, from
two truncations applied after the label check:

1. **Per-pattern balance.** Truncate each pattern's longer side
   (matches or non-matches) to the shorter side's count, keeping the
   first examples in original order. Fewer than one of each makes the
   pattern unusable.
2. **Dataset-wide balance.** Truncate every usable pattern further
   down to the smallest per-pattern count (`balance_k_global`). On the
   real corpus this is expensive: one pattern balanced to `1` forces
   every other pattern down to `1` match + `1` non-match too.
   `dataset_prepare` prints the actual `global balance k`.

Two accounting notes: a rule byte-identical to an earlier one's raw
pattern text is skipped before translation (`patterns_duplicate`),
same column `dataset_stats` reports on the raw corpus; and summary
numbers are **appended**, one row per run, to
`reports/data/dataset_prepare.csv` -- not replaced, so re-runs
accumulate (archive or clear the file first if comparing across a
schema change).

## Pipeline

1. `extract::extract_s2_elab` turns the raw JSON into `ExtractedRule`s
   (pattern + the author's own `matches`/`non_matches`).
2. Dedup by raw pattern text against every rule already seen this run
   (`patterns_duplicate`, skipped).
3. `prepare::core_regex_tagged` parses and translates, with two size
   caps and one narrow anchor recovery. Rejected patterns stop here.
4. A small dedup/positive-wins helper turns `matches`/`non_matches`
   into `(text, is_match)` pairs, unchanged, uncapped, unsampled.
5. `prepare::matches_bounded` re-checks each pair against the real
   `Regex`; a disagreement is dropped (`inputs_label_mismatch`).
6. Per-pattern balance: longer side truncated to the shorter side's
   count (`inputs_dropped_for_pattern_balance`).
7. Dataset-wide balance: every usable pattern truncated again, to the
   smallest per-pattern count (`balance_k_global`,
   `inputs_dropped_for_global_balance`).
8. Each surviving pair becomes a `PreparedCase`, written as JSON lines
   tagged with its `MatchIntent`.
9. `run_pipeline` drives 1-8 and returns `PipelineReport`;
   `load_prepared_cases` reads the result back for benchmarks.

## Match-intent

A pattern is accepted one of two ways, recorded on every
`PreparedCase`:

| `MatchIntent` | Pattern | Why |
|---|---|---|
| `ExplicitFullMatch` | Wrapped in `^...$` at its own top level | Anchors are provably redundant under full-string matching, so stripped before translation instead of rejected |
| `UnanchoredAmbiguous` | No anchor anywhere | Benchmarked as a full match -- this project's choice, not the source tool's native convention |

`ExplicitFullMatch` is narrow on purpose: a lone `^`/`$`, or one nested
in an `Or` branch or under a quantifier, is left untouched and
rejected by `translate`. `UnanchoredAmbiguous` covers the rest --
Suricata/SpamAssassin search a buffer by default, and RegexLib's
unanchored entries mix forgotten anchors with deliberate substring
checks.

**Default scope.** The benches default `BENCH_MATCH_INTENT` to
`explicit` (`ExplicitFullMatch` only): an unanchored pattern's intended
reading (search vs. full match) is genuinely ambiguous, so defaulting
to benchmarking it would overstate how settled that reading is.
`UnanchoredAmbiguous` is opt-in via `BENCH_MATCH_INTENT=direct`/`both`.

**Positives vs. negatives.** A positive holds under either reading (a
string in the pattern's own language is trivially found by a search
over that string). A negative does not automatically hold for
`UnanchoredAmbiguous`: a string that fails full-match semantics can
still contain a valid match earlier under search semantics.
`run_pipeline` does not check for this -- every given negative is
written as-is, trusting the dataset's own label. Only
`UnanchoredAmbiguous` negatives are affected.

**Benchmark cap.** The benches and `demo_parser_agreement` admit every
distinct pattern per source by default; `BENCH_PATTERN_LIMIT=N` caps
to the first N distinct patterns (not rows).

## Pathological stress patterns

A second, unrelated dataset at
`data/pathological/pathological_patterns.json`: 107 hand-curated rows
(56 `pathological_patterns` + 43 `baseline_and_edge_patterns` + 8
`generated_patterns`) designed to stress this project's own derivative
engines, not backtracking blowup. Curated input, not pipeline output
-- tracked like `RegexEval.json`, read directly by
`bench_pathological_time`/`memory`/`translate` (plus row `E51`
specifically by `bench_stack_depth`, see
[BENCHMARKS.md](BENCHMARKS.md)), never through `run_pipeline`.
`src/data/pathological.rs` has the loader and the
`{N}`/`{P}`-template expansion.

The file's own `meta.purpose`/`meta.engine_notes` fields: "pathological"
here means derivative-state explosion or polynomial/compile blow-up
under this engine family, not backtracking blowup (catastrophic
backtracking does not apply); every pattern is full-string-match only
-- anchors, `\b`/`\B`, backreferences, lookaround, and POSIX classes
all hard-reject at parse/translate time in this project's own frontend,
verified directly against it, so none appear in the dataset.

Row count alone is misleading: `suggested_n`/`suggested_p` sweeps mean
one row expands into several entries.

```bash
cargo run --release --example pathological_stats
```

Saves `reports/data/pathological_stats.csv` (per category: patterns,
variable-size patterns, input sizes, pattern sizes, test cases under
the default sweep) and `reports/data/pathological_rows.csv` (per
pattern). As of this writing: 113 patterns, 811 test cases under the
default sweep -- category `L` alone (33 baseline/edge patterns)
accounts for 195 of them.

### Categories

Every row belongs to one of 11 categories (`A`-`L`, skipping `G`):

| Category | Name | Mechanism | Stress axis | suggested_n | suggested_p |
|---|---|---|---|---|---|
| A | Nested quantifiers | Quantified group inside another quantifier (`+`,`*`,`{1,}`) | derivative-state explosion | 20, 50, 100, 200 | -- |
| B | Overlapping alternation | Alternatives sharing a prefix under a quantifier | derivative-state explosion | 20, 50, 100, 200 | -- |
| C | Bounded rep over star | `{m,n}` wrapping a `*`/`+` body, unrolled at translate time | compile blow-up + polynomial match | 100, 500, 1000, 5000 | -- |
| D | Anchored-failure style | Must fully consume input; near-miss tail forces full enumeration | compile blow-up + derivative-state explosion | 40, 60, 100 | -- |
| E | Prefix-overlap alternation | One alternative is a strict prefix of another | derivative-state explosion | 20, 50, 100, 200 | -- |
| F | Alternation-star + literal | `(alt)*`/`(alt)+` then a literal that fails late | polynomial or derivative-state explosion | 100, 500, 1000 | -- |
| H | Many adjacent stars/groups | Product of `.+`, `.*`, `\w+` etc. in sequence | polynomial O(N^k), k = star count | 10, 20, 50, 100, 500, 1500 | -- |
| I | Trivial pattern, large input | Simple pattern, very long input | linear throughput | 100000, 1000000, 10000000 | -- |
| J | Large pattern, trivial input | Thousands of literals/groups, short input | compile blow-up (pattern size) | -- | 100, 1000, 10000, 100000 |
| K | Large pattern, large input | Both axes at once | compile + match blow-up | 500, 1000, 5000, 10000, 50000 | 1000 |
| L | Trivial pattern, trivial input | Baseline/sanity check | linear throughput | 10, 100, 1000 | -- |

`I`/`J`/`K`/`L` form a 2x2 matrix of (pattern size: trivial/large) x
(input size: trivial/large), `L` the all-trivial baseline.
`suggested_n`/`suggested_p` are each category's recommended sweep
points; a row with no `suggested_p` has a fixed pattern size, a row
with no `suggested_n` (only `J`) has a fixed input size.

Two exceptions worth knowing before sweeping a specific row:

- **D's rows don't currently scale with `PATHO_N` at all.** `P43`/
  `P44`/`P45`/`P57` each use a fixed-integer `match_input`/
  `nomatch_input` length (e.g. exactly 40 characters), not a `{N}`
  template -- they test a specific near-miss boundary, not a sweepable
  curve. `suggested_n` is listed for completeness but has no effect on
  any row in this category today; there's no `(N, time)` curve to get
  out of D as it stands.
- **K's upper range can genuinely crash `deriv_std_rec`/`deriv_std_loop`
  uncontained**, confirmed directly: at `n=50000` on the large-pattern
  generator rows (`E41`/`E52`, whose *pattern* is already ~1000 words),
  both std parsers can exhaust memory for real -- not a timeout, an
  actual OS-level crash, exactly the failure mode `BENCH_MEM_CAP_MB`
  (see [BENCHMARKS.md](BENCHMARKS.md)) exists to contain. Always set it
  when sweeping K at its full range; this category combines both stress
  axes on purpose, so reaching a real crash point is the intended
  result, not a bug -- it just needs to happen inside a cap.

To load and sweep this dataset, see `PATHO_JSON_PATH`, `PATHO_SECTION`,
`PATHO_CATEGORY`, `PATHO_N`, `PATHO_P` in [BENCHMARKS.md](BENCHMARKS.md).
