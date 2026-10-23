# Datasets

Two pattern sources:

- **S2ELab** — real human-authored regex benchmark with its own given
  test inputs.
- **Pathological stress patterns** — hand-curated patterns with generated
  inputs, no author-given examples (see
  [Pathological stress patterns](#pathological-stress-patterns) below).

Suricata/Snort, SpamAssassin, RegexLib and Innovatorved are still read
by `dataset_stats`'s raw-corpus diagnostics for historical comparison,
but no longer feeding `dataset_prepare`/`run_pipeline`, and not used by
any benchmark.


## S2ELab

A human-authored regex benchmark: 762 patterns, each contributed with a
natural-language description and the author's own labeled examples. The
file lives at `data/raw/s2-elab/RegexEval.json` (one JSON array of 762
entries) and was taken of `DatasetCollection/RegexEval.json` from
[github.com/s2e-lab/RegexEval](https://github.com/s2e-lab/RegexEval).

One entry (`id: 100`), fields trimmed:

```json
{
  "expression": "^(0[1-9]|1[0-2])\/((0[1-9]|2\\d)|3[0-1])\/(19\\d\\d|200[0-3])$",
  "matches": ["12/31/2003", "01/01/1900", "11/31/2002", "10/31/2003", "..."],
  "non_matches": ["1/1/2002", "01/01/02", "01/01/2004", "12/123/4322", "..."]
}
```

That is the corpus's shape: real patterns with real author-given labels,
sometimes debatable but always what a human intended. `run_pipeline`
trusts each entry's own `matches`/`non_matches` wherever they agree with
the real translated `Regex`; nothing else about S2ELab's labels is
re-checked.



## Dataset statistics

```bash
# No args: all default sources under data/raw/
cargo run --release --example dataset_stats

# --verbose: list each rejected pattern next to its cause
cargo run --release --example dataset_stats -- --verbose --source s2-elab

# --source: restrict to one or more sources
cargo run --release --example dataset_stats -- \
  --source suricata --source spamassassin --source regexlib --source s2-elab --source innovatorved
```

Flags: `--source S` restrict to one source, `--verbose` list each
rejected pattern next to its cause, `--no-flags` suppress the two flag
tables (and CSVs).

Below is a run of all 5 sources. 


**Dataset coverage**
- **Accepted / Rejected** count distinct raw pattern text, not rule instances.
- **Dup** is how many extracted instances that dedup removed.
- **Inputs Provided** is S2ELab's own given `matches` + `non_matches`; 0 for every other source.

| Source | Files | Extracted | Inputs Provided | Dup | Accepted | Acc% | Rejected | Rej% |
|---|---|---|---|---|---|---|---|---|
| snort | 47 | 16774 | 0 | 6042 | 2082 | 34.0 | 8650 | 81.2 |
| spamassassin | 62 | 549 | 0 | 13 | 155 | 95.7 | 381 | 98.4 |
| regexlib | 2 | 6506 | 0 | 3055 | 2172 | 52.0 | 1279 | 54.8 |
| s2-elab | 1 | 762 | 18930 | 7 | 578 | 98.8 | 177 | 100.0 |
| innovatorved | 1 | 8549 | 0 | 340 | 4803 | 95.6 | 3406 | 96.6 |
| **total** | **113** | **33140** | **18930** | **10186** | **9230** | **57.4** | **13724** | **80.4** |



**Accepted by match-intent reliability** 

| Source | UnanchoredAmbiguous | % | ExplicitFullMatch `^...$` | % |
|---|---|---|---|---|
| snort | 1623 | 78.0 | 459 | 22.0 |
| spamassassin | 132 | 85.2 | 23 | 14.8 |
| regexlib | 881 | 40.6 | 1291 | 59.4 |
| s2-elab | 115 | 19.9 | 463 | 80.1 |
| innovatorved | 3358 | 69.9 | 1445 | 30.1 |
| **total** | **6000** | **65.0** | **3230** | **35.0** |

`Direct` = no anchor, translated as-is. `Explicit ^...$` = top-level wrap,
anchors stripped; any other placement is rejected.



**Rejection reasons** (one column per cause `core_regex_tagged` gave)

| Source | backreference | lookaround | word-boundary | anchor | flag-m | flag-x | other |
|---|---|---|---|---|---|---|---|
| snort | 215 | 98 | 19 | 6264 | 3963 | 0 | 92 |
| spamassassin | 8 | 50 | 164 | 142 | 19 | 0 | 4 |
| regexlib | 69 | 594 | 153 | 901 | 9 | 0 | 607 |
| s2-elab | 3 | 43 | 23 | 106 | 0 | 0 | 2 |
| innovatorved | 154 | 1324 | 239 | 1052 | 2 | 0 | 754 |
| **total** | **449** | **2109** | **598** | **8465** | **3993** | **0** | **1459** |



**Anchor shape** (every extracted pattern, accepted or not)
- **Unparseable**: rejected before an `ExtPat` even exists.
- **Fully-wrapped**: recovered by `core_regex_tagged`.
- **start / end / nested**: rejected.

| Source | unparseable | none | fully-wrapped | start-only | end-only | nested |
|---|---|---|---|---|---|---|
| snort | 372 | 5739 | 2783 | 5934 | 1669 | 277 |
| spamassassin | 63 | 304 | 34 | 136 | 8 | 4 |
| regexlib | 1232 | 1766 | 2622 | 197 | 98 | 591 |
| s2-elab | 48 | 138 | 472 | 16 | 8 | 80 |
| innovatorved | 2170 | 3745 | 1580 | 445 | 195 | 414 |
| **total** | **3885** | **11692** | **7491** | **6728** | **1978** | **1366** |



## Dataset preparation

No input is generated, derived, or heuristically assembled. `run_pipeline`
is **S2ELab-only**: it extracts and translates patterns, then writes each
usable pattern's own `matches`/`non_matches`. Every given example is run
against the real translated `Regex` via `matches_bounded`; disagreements
are dropped (`inputs_label_mismatch`). Suricata/SpamAssassin/RegexLib/
Innovatorved are still read by `dataset_stats`, never by `run_pipeline`.

```bash
cargo run --release --example dataset_prepare -- data/raw/s2-elab/RegexEval.json
```

Writes `data/processed/s2-elab/prepared.jsonl`, replacing the file.

**Every usable pattern contributes the same total row count**, from two
truncations applied after the label check:

1. **Per-pattern balance.** Truncate each pattern's longer side (matches
   or non-matches) down to the shorter side's count, keeping the first
   examples in original order. Fewer than one of each makes the pattern
   unusable.
2. **Dataset-wide balance.** Truncate every usable pattern further down to
   the smallest per-pattern count (`balance_k_global`).

On the real S2ELab corpus this is expensive: as long as one pattern's own
balanced count is `1`, every other pattern is truncated to `1` match + `1`
non-match too. `dataset_prepare` prints the actual `global balance k`;
summary numbers are appended to `reports/data/dataset_prepare.csv`.

Two accounting notes:

- **Dedup.** A rule byte-identical to an earlier one's raw pattern text is
  skipped before translation (`patterns_duplicate`); `dataset_stats`'s
  `Dup` column reports the same on the raw corpus.
- **CSV history.** Summary numbers are *appended* one row per run. Four
  balance columns are new this version (`inputs_dropped_for_pattern_balance`,
  `balance_k_global`, `inputs_dropped_for_global_balance`,
  `patterns_truncated_for_global_balance`); archive or clear the old file
  before comparing across the change.

The same run's summary numbers are *appended* as one row to
`reports/data/dataset_prepare.csv`, not replaced, so re-runs accumulate.
The CSV gained four balance columns this version
(`inputs_dropped_for_pattern_balance`, `balance_k_global`,
`inputs_dropped_for_global_balance`,
`patterns_truncated_for_global_balance`); archive or clear the old file
before comparing across the change if that matters.


## Pipeline

1. `extract::extract_s2_elab` turns one raw file's JSON into
   `ExtractedRule`s: pattern plus the author's own `matches`/`non_matches`.
2. Dedup by raw pattern text against every rule already seen in this run.
   Repeats counted (`patterns_duplicate`) and skipped.
3. `prepare::core_regex_tagged` parses and translates, with two size caps
   and one narrow anchor recovery. Rejected patterns stop here.
4. A small dedup/positive-wins helper in `mod.rs` turns
   `matches`/`non_matches` into `(text, is_match)` pairs, unchanged,
   uncapped, unsampled.
5. `prepare::matches_bounded` re-checks each pair against the real
   `Regex`; a claim it disagrees with is dropped (`inputs_label_mismatch`).
6. Per-pattern balance: the longer side is truncated to the shorter
   side's count (`inputs_dropped_for_pattern_balance`). Fewer than one of
   each after this makes the pattern unusable.
7. Dataset-wide balance: every usable pattern is truncated again, down to
   the smallest per-pattern count (`balance_k_global`,
   `inputs_dropped_for_global_balance`).
8. Each surviving pair becomes a `PreparedCase`, written as JSON lines
   tagged with its `MatchIntent`.
9. `run_pipeline` drives 1–8 and returns `PipelineReport`;
   `load_prepared_cases` reads the result back for benchmarks.


## Match-intent

A pattern is accepted in one of two ways, recorded on every `PreparedCase`:

| `MatchIntent` | Pattern | Why |
|---|---|---|
| `ExplicitFullMatch` | Wrapped in `^...$` at its own top level | Anchors are provably redundant under full-string matching, so `core_regex_tagged` strips them before translation instead of rejecting |
| `UnanchoredAmbiguous` | No anchor anywhere | Benchmarked as a full match — this project's choice, not the source tool's native convention |

`ExplicitFullMatch` is narrow on purpose: a lone `^` or `$`, or one
nested in an `Or` branch or under a quantifier, is left untouched and
rejected by `translate`. `UnanchoredAmbiguous` covers the rest: Suricata
and SpamAssassin search a buffer by default, and RegexLib's unanchored
entries mix forgotten anchors with deliberate substring checks.

**Default scope.** The benches (`bench_corpus_time`, `bench_memory`,
`bench_translate`) default `BENCH_MATCH_INTENT` to `explicit`, i.e.
`ExplicitFullMatch` only. `UnanchoredAmbiguous` is opt-in via
`BENCH_MATCH_INTENT=direct` or `=both`. The reasoning: an unanchored
pattern's intended reading is ambiguous (search vs. full match), so
benchmarking it by default overstates how settled that reading is;
`ExplicitFullMatch` carries no such ambiguity.

**Positives vs negatives.** A positive is sound under either reading: a
string in the pattern's own language is trivially found by a search over
that string. A negative is not automatically so for an
`UnanchoredAmbiguous` pattern: a string that fails under full-match
semantics can still contain a valid match earlier in the same string
under search semantics. `run_pipeline` does not check for this; every
given negative is written as-is, trusting the dataset's own label. Only
`UnanchoredAmbiguous` negatives are affected.

**Benchmark cap.** `bench_corpus_time`/`bench_memory`/`bench_translate`
and `demo_parser_agreement` admit every distinct pattern per source by
default; `BENCH_PATTERN_LIMIT=N` caps to the first N. The cap is on
distinct patterns, not rows.


## Pathological stress patterns

A second, unrelated dataset at
`data/pathological/pathological_patterns.json`: 107 hand-curated rows
(56 `pathological_patterns` + 43 `baseline_and_edge_patterns` + 8
`generated_patterns`) designed to stress this project's own derivative
engines, not backtracking blowup. Tracked like
`data/raw/s2-elab/RegexEval.json` (curated input, not pipeline output),
read directly by `bench_pathological_time`, `bench_pathological_memory`
and `bench_pathological_translate`, plus row `E51` specifically by
`bench_stack_depth` as a state-blowup-free recursion-depth control
(see [BENCHMARKS.md](BENCHMARKS.md)), never through
`run_pipeline`/`prepared.jsonl`. `src/data/pathological.rs` has the
loader and the `{N}`/`{P}`-template expansion.

The file's own `meta.purpose` and `meta.engine_notes` fields say what
"pathological" means here and why no anchor/backreference/lookaround
shapes appear:

> **purpose**: Stress-test patterns for the derivative-based matcher
> (Brzozowski/Antimirov, deriv_std/pderiv engines). "Pathological" here
> means derivative-state explosion or polynomial/compile blow-up under
> this engine family, not backtracking blowup — catastrophic backtracking
> does not apply.
>
> **engine_notes**: All patterns verified against the real
> `src/frontend/{parse,translate,alphabet}.rs` on the benches branch:
> anchors, `\b`/`\B`, backreferences, lookaround and POSIX classes all
> hard-reject at parse/translate time, so none appear below. Every pattern
> is full-string-match only.

Row count alone is misleading: `suggested_n`/`suggested_p` sweeps mean
one row expands into several entries. Run `pathological_stats` for the
real breakdown:

```bash
cargo run --release --example pathological_stats
```

Saves `reports/data/pathological_stats.csv` (per category: rows,
generator rows, `suggested_n`/`suggested_p`, swept entries under the
default sweep) and `reports/data/pathological_rows.csv` (per row). As
of this writing: 107 rows, 723 entries under the default sweep --
category `L` alone (33 baseline/edge rows) accounts for 195 of them.

### Categories

Every row belongs to one of 11 categories (`A`–`L`, skipping `G`), read
straight out of the JSON's own `categories` object:

| Category | Name | Mechanism | Stress axis | suggested_n | suggested_p |
|---|---|---|---|---|---|
| A | Nested quantifiers | Quantified group inside another quantifier (`+`, `*`, `{1,}`) | derivative-state explosion | 20, 50, 100, 200 | — |
| B | Overlapping alternation | Alternatives matching the same prefix under a quantifier | derivative-state explosion | 20, 50, 100, 200 | — |
| C | Bounded rep over star | `{m,n}` wrapping a `*`/`+` body; unrolled at translate time | compile blow-up (unrolled) + polynomial match | 100, 500, 1000, 5000 | — |
| D | Anchored-failure style | Must fully consume input; near-miss tail forces full enumeration | compile blow-up (unrolled) + derivative-state explosion | 40, 60, 100 | — |
| E | Prefix-overlap alternation | One alternative is a strict prefix of another | derivative-state explosion | 20, 50, 100, 200 | — |
| F | Alternation-star + literal | `(alt)*` or `(alt)+` then a literal that fails late | polynomial (simple alt) or derivative-state explosion (nested) | 100, 500, 1000 | — |
| H | Many adjacent stars/groups | Product of `.+`, `.*`, `\w+` etc. in sequence | polynomial O(N^k), k = number of stars | 100, 500, 1000, 5000 | — |
| I | Huge input, trivial pattern | Simple pattern on a very long input | linear throughput / constant-factor | 100000, 1000000, 10000000 | — |
| J | Large pattern, trivial input | Thousands of literals/groups; short input | compile blow-up (pattern size) | — | 100, 1000, 10000, 100000 |
| K | Large pattern + large input | Both axes simultaneously | compile + match blow-up | 1000 | 1000 |
| L | Baseline / easy | Simple pattern, simple input | linear throughput — sanity/warmup | 10, 100, 1000 | — |

`suggested_n`/`suggested_p` are each category's recommended sweep points
(input length / pattern size). A row with no `suggested_p` has a fixed
pattern size; a row with no `suggested_n` (only `J`) has a fixed input
size.

To load and sweep this dataset, see the `PATHO_JSON_PATH`,
`PATHO_SECTION`, `PATHO_CATEGORY`, `PATHO_N`, `PATHO_P` env vars in
[BENCHMARKS.md](BENCHMARKS.md); not duplicated here.