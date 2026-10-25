# Datasets

Two pattern sources.

- **S2ELab** is a real human-authored regex benchmark with its own given test inputs.
- **Pathological stress patterns** are hand-curated patterns with generated inputs. No author-given examples. See [Pathological stress patterns](#pathological-stress-patterns) below.

Suricata, Snort, SpamAssassin, RegexLib and Innovatorved feed only `dataset_stats`'s raw-corpus diagnostics. They no longer feed `dataset_prepare` or `run_pipeline`, and no benchmark reads them.


## S2ELab

762 patterns. Each one has a natural-language description and the author's own labeled examples. Lives at `data/raw/s2-elab/RegexEval.json`, taken from [github.com/s2e-lab/RegexEval](https://github.com/s2e-lab/RegexEval).

One entry (`id: 100`), fields trimmed.

```json
{
  "expression": "^(0[1-9]|1[0-2])\/((0[1-9]|2\\d)|3[0-1])\/(19\\d\\d|200[0-3])$",
  "matches": ["12/31/2003", "01/01/1900", "11/31/2002", "10/31/2003", "..."],
  "non_matches": ["1/1/2002", "01/01/02", "01/01/2004", "12/123/4322", "..."]
}
```

`run_pipeline` reads each entry's own `matches` and `non_matches` and keeps every case whose outcome agrees with the translated `Regex`.


### Dataset statistics

```bash
cargo run --release --example dataset_stats
```

Flags are `--source S` to restrict to one source, `--verbose` to list each rejected pattern next to its cause, and `--no-flags` to suppress the two flag tables and their CSVs.

The run below covers all five sources.

**Dataset coverage.** Accepted and Rejected count distinct raw pattern text, not rule instances. Dup is how many extracted instances dedup removed. Inputs Provided is S2ELab's own given `matches` plus `non_matches`. It is 0 for every other source.

| Source | Files | Extracted | Inputs Provided | Dup | Accepted | Acc% | Rejected | Rej% |
|---|---|---|---|---|---|---|---|---|
| snort | 47 | 16774 | 0 | 6042 | 2082 | 34.0 | 8650 | 81.2 |
| spamassassin | 62 | 549 | 0 | 13 | 155 | 95.7 | 381 | 98.4 |
| regexlib | 2 | 6506 | 0 | 3055 | 2172 | 52.0 | 1279 | 54.8 |
| s2-elab | 1 | 762 | 18930 | 7 | 578 | 98.8 | 177 | 100.0 |
| innovatorved | 1 | 8549 | 0 | 340 | 4803 | 95.6 | 3406 | 96.6 |
| **total** | **113** | **33140** | **18930** | **10186** | **9230** | **57.4** | **13724** | **80.4** |

**Accepted by match-intent reliability.** `UnanchoredAmbiguous` means no anchor, translated as-is. `ExplicitFullMatch` means a top-level `^...$` wrap, anchors stripped. Any other anchor placement is rejected.

| Source | UnanchoredAmbiguous | % | ExplicitFullMatch `^...$` | % |
|---|---|---|---|---|
| snort | 1623 | 78.0 | 459 | 22.0 |
| spamassassin | 132 | 85.2 | 23 | 14.8 |
| regexlib | 881 | 40.6 | 1291 | 59.4 |
| s2-elab | 115 | 19.9 | 463 | 80.1 |
| innovatorved | 3358 | 69.9 | 1445 | 30.1 |
| **total** | **6000** | **65.0** | **3230** | **35.0** |

**Rejection reasons.** One column per cause `core_regex_tagged` reported.

| Source | backreference | lookaround | word-boundary | anchor | flag-m | flag-x | other |
|---|---|---|---|---|---|---|---|
| snort | 215 | 98 | 19 | 6264 | 3963 | 0 | 92 |
| spamassassin | 8 | 50 | 164 | 142 | 19 | 0 | 4 |
| regexlib | 69 | 594 | 153 | 901 | 9 | 0 | 607 |
| s2-elab | 3 | 43 | 23 | 106 | 0 | 0 | 2 |
| innovatorved | 154 | 1324 | 239 | 1052 | 2 | 0 | 754 |
| **total** | **449** | **2109** | **598** | **8465** | **3993** | **0** | **1459** |

**Anchor shape.** Covers every extracted pattern, accepted or not. Unparseable means rejected before an `ExtPat` even exists. Fully-wrapped means recovered by `core_regex_tagged`. The start, end, and nested columns are rejected.

| Source | unparseable | none | fully-wrapped | start-only | end-only | nested |
|---|---|---|---|---|---|---|
| snort | 372 | 5739 | 2783 | 5934 | 1669 | 277 |
| spamassassin | 63 | 304 | 34 | 136 | 8 | 4 |
| regexlib | 1232 | 1766 | 2622 | 197 | 98 | 591 |
| s2-elab | 48 | 138 | 472 | 16 | 8 | 80 |
| innovatorved | 2170 | 3745 | 1580 | 445 | 195 | 414 |
| **total** | **3885** | **11692** | **7491** | **6728** | **1978** | **1366** |


### Dataset preparation

No input is generated, derived, or heuristically assembled. `run_pipeline` is S2ELab-only. It extracts and translates patterns, then writes each usable pattern's own `matches` and `non_matches` back out. Every case is re-checked against the real translated `Regex` via `matches_bounded`. A disagreement is dropped and counted under `inputs_label_mismatch`.

```bash
cargo run --release --example dataset_prepare -- data/raw/s2-elab/RegexEval.json
```

Writes `data/processed/s2-elab/prepared.jsonl`, replacing the file.

**Per-pattern balance.** After the label check, each pattern's longer side (matches or non-matches) is truncated to its shorter side's count, keeping the first examples in original order. Fewer than one of each makes the pattern unusable. Patterns are not truncated against each other. A pattern with 50 given examples keeps all 50, even if another pattern has only 1.

Per-pattern balance is local. A single pattern is not balanced against the dataset-wide minimum. `bench_corpus_time.rs` avoids cross-pattern weighting with macro-averaging (see that file's module doc), so it does not need the corpus trimmed to the thinnest pattern.

**Accounting notes.** A rule byte-identical to an earlier one's raw pattern text is skipped before translation (`patterns_duplicate`). The summary numbers append to `reports/data/dataset_prepare.csv`, one row per run. Re-runs accumulate. Archive or clear the file first when comparing across a schema change.


### Pipeline

1. `extract::extract_s2_elab` turns the raw JSON into `ExtractedRule` values, each carrying the pattern plus the author's own `matches` and `non_matches`.
2. Dedup by raw pattern text against every rule already seen this run (`patterns_duplicate`, skipped).
3. `prepare::core_regex_tagged` parses and translates with two size caps and one narrow anchor recovery. Rejected patterns stop here.
4. A small dedup helper with positive-wins resolves `matches` and `non_matches` into `(text, is_match)` pairs. Unchanged, uncapped, unsampled.
5. `prepare::matches_bounded` re-checks each pair against the real `Regex`. A disagreement is dropped (`inputs_label_mismatch`).
6. Per-pattern balance. The longer side is truncated to the shorter side's count (`inputs_dropped_for_pattern_balance`).
7. Each surviving pair becomes a `PreparedCase`, written as JSON lines tagged with its `MatchIntent`.
8. `run_pipeline` drives steps 1 to 7 and returns a `PipelineReport`. `load_prepared_cases` reads the result back for benchmarks.


### Match-intent

Each `PreparedCase` carries the reading under which it was accepted.

| `MatchIntent` | Trigger | Reading |
|---|---|---|
| `ExplicitFullMatch` | Top-level `^...$` wrap | Full match. Anchors stripped before translation. |
| `UnanchoredAmbiguous` | No anchor anywhere | Full match under this project's convention. |

Only a top-level `^...$` wrap counts. A lone `^` or `$`, one nested in an `Or` branch, or one under a quantifier is rejected by `translate`.

**Default scope.** `BENCH_MATCH_INTENT=explicit` is the default. It selects `ExplicitFullMatch` only. `UnanchoredAmbiguous` is opt-in via `direct` or `both`.

**Positives and negatives.**

| Case | `ExplicitFullMatch` | `UnanchoredAmbiguous` |
|---|---|---|
| Positive | Holds | Holds |
| Negative | Holds | May fail. A substring can still match earlier in a search. |

`run_pipeline` does not re-check negatives under `UnanchoredAmbiguous`. Each given negative is written as-is.

**Benchmark cap.** `BENCH_PATTERN_LIMIT=N` caps to the first N distinct patterns per source, not rows.


---
## Pathological stress patterns

`data/pathological/pathological_patterns.json` holds 113 hand-curated rows.

| Section | Rows | Purpose |
|---|---|---|
| `pathological_patterns` | 61 | Main stress rows |
| `baseline_and_edge_patterns` | 43 | Baseline and edge cases |
| `generated_patterns` | 9 | Parameterised by `{N}` and `{P}` templates |

Read directly by `bench_pathological_time`, `bench_pathological_memory`, `bench_pathological_translate`, and row `E51` only by `bench_stack_depth`. Never read by `run_pipeline`. Loader and template expansion in `src/data/pathological.rs`.

**Scope.** Pathological here means derivative-state explosion, polynomial blow-up, or compile blow-up under this engine family. Not catastrophic backtracking. Every row is full-string-match only. Anchors, `\b`, `\B`, backreferences, lookaround, and POSIX classes hard-reject at parse or translate time, so no row contains one.

**Test surface.** Rows expand into more entries than their raw count suggests. `suggested_n` and `suggested_p` sweeps multiply each row into several test cases.

```bash
cargo run --release --example pathological_stats
```

Writes `reports/data/pathological_stats.csv` (one row per category) and `reports/data/pathological_rows.csv` (one row per pattern).

Current totals are 113 patterns and 913 test cases under the default sweep. Category `L` alone (33 baseline and edge patterns) accounts for 195 cases.

### Categories

Every row belongs to one of 11 categories, `A` through `L` with `G` skipped.

`suggested_n` and `suggested_p` for `A`, `B`, `C`, `D`, `E`, `F`, and `H` are calibrated. Each range is chosen from a real measured sweep so that a routine, un-overridden run crosses from "every parser is fast" to "`deriv_std` is gone" within the category's own default points. Each category's `suggested_n_notes` field in the JSON records the calibration behind the range.

| Category | Name | Mechanism | Stress axis | suggested_n | suggested_p |
|---|---|---|---|---|---|
| A | Nested quantifiers | Quantified group inside another quantifier (`+`, `*`, `{1,}`) | derivative-state explosion | 10, 20, 30, 1000, 10000 | none |
| B | Overlapping alternation | Alternatives sharing a prefix under a quantifier | derivative-state explosion | 10, 20, 30, 1000, 10000 | none |
| C | Bounded rep over star | `{m,n}` wrapping a `*` or `+` body, unrolled at translate time | compile blow-up plus polynomial match | 10, 50, 200, 1000 | none |
| D | Anchored-failure style | Must fully consume input. A near-miss tail forces full enumeration. | compile blow-up plus derivative-state explosion | 10, 20, 30, 50, 75 | none |
| E | Prefix-overlap alternation | One alternative is a strict prefix of another | derivative-state explosion | 10, 100, 1000, 5000, 50000 | none |
| F | Alternation-star plus literal | `(alt)*` or `(alt)+` then a literal that fails late | polynomial or derivative-state explosion | 10, 100, 1000, 10000, 50000 | none |
| H | Many adjacent stars or groups | Product of `.+`, `.*`, `\w+` and similar in sequence | polynomial O(N^k), k is star count | 10, 50, 200, 1000 | none |
| I | Trivial pattern, large input | Simple pattern, very long input | linear throughput | 100000, 1000000, 10000000 | none |
| J | Large pattern, trivial input | Thousands of literals or groups, short input | compile blow-up on pattern size | none | 100, 500, 1000, 2000 |
| K | Large pattern, large input | Both axes at once | compile plus match blow-up | 500, 1000, 5000, 10000, 50000 | 300 |
| L | Trivial pattern, trivial input | Baseline, sanity check | linear throughput | 10, 100, 1000 | none |

`I`, `J`, `K`, and `L` form a 2x2 matrix of pattern size (trivial or large) against input size (trivial or large). `L` is the all-trivial baseline.

A row with no `suggested_p` has a fixed pattern size. Only `J` has no `suggested_n`, reflecting its fixed input size.

### Category notes

| Category | Note |
|---|---|
| `C`, `H` | Sweep ceilings are lower than the blow-up categories on purpose. At `n=100` these rows cost ~0.7-0.9s per call for `deriv_bc` and `pderiv_std`. At `n=1000` they cost ~7-10s. Every parser times out before `n=5000`. Higher values produce more timeouts without more signal. |
| `K` | Upper range can crash `deriv_std_rec` and `deriv_std_loop`. `E41` runs at fixed `p=1000`, so its pattern is around 1000 alternation branches. At `n=50000`, both std parsers can exhaust memory on it. That is an OS-level crash, not a timeout. Always run `K` at full range under `BENCH_MEM_CAP_MB` (see [BENCHMARKS.md](BENCHMARKS.md)). A crash inside the cap is the expected outcome at the top of this range. |
| `J`, `K` | `suggested_p` values stay under `core_regex_tagged`'s 20,000-node cap. Higher values push every parser through the same translation-level failure and report a uniform `crashed` count, which is not a parser-timing result. |

### Rows without a nomatch_input

Two category `H` rows use a different shape from their siblings.

- `P29` (`(.+){10}`) uses a fixed 9-character input.
- `P31` (`(.*){10}`) carries no `nomatch_input` at all.

`.` matches any single character, so a pattern built from ten unbounded `.+` or `.*` groups accepts every input of ten characters or more. A non-matching input has to be shorter than the pattern's own group minimum. `P29` therefore sweeps a 9-character input. `P31` accepts the empty string and every extension of it, so no input can fail to match and the row carries no `nomatch_input`.

To load and sweep this dataset, see `PATHO_JSON_PATH`, `PATHO_SECTION`, `PATHO_CATEGORY`, `PATHO_N`, and `PATHO_P` in [BENCHMARKS.md](BENCHMARKS.md).