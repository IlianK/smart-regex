# Benchmarks

## Benchmark REGEX Simple

Criterion benchmarks under `benches/`.

```bash
# Run all benchmarks (HTML report: target/criterion/report/index.html)
cargo bench
cargo bench -- --verbose

# Specific benchmark file
cargo bench --bench bench_match
cargo bench --bench bench_posix

# Specific group within a file
cargo bench --bench bench_posix -- scaling_a_star
cargo bench --bench bench_posix -- ambiguous_star_a_or_aa
```


## Benchmark REGEX Datasets (Mamouras et al. (PLDI 2024))

Small, real samples from the three dataset families the Mamouras et al.
(PLDI 2024) paper draws from (Snort/Suricata, SpamAssassin, RegexLib) --
the paper itself provides no public artifact/dataset, so these are
independently sourced.



## Sources

| Directory/file | Source | Fetched from | License / terms |
|---|---|---|---|
| `raw/emerging-exploit.rules`, `raw/emerging-web_client.rules` | Suricata/Snort `pcre:` rules, Emerging Threats OPEN ruleset | [Suratica Github](github.com/seanlinmt/suricata) (mirror of the ET Open ruleset) | ET Open rules are freely distributable for IDS use; see [rules.emergingthreats.net](https://rules.emergingthreats.net/OPEN_download_instructions.html) for the canonical source and current terms |
| `raw/20_drugs.cf`, `raw/20_body_tests.cf`, `raw/20_head_tests.cf` | SpamAssassin default rules | [SpamAssassin Github](github.com/apache/spamassassin) (official read-only mirror), `rules/` dir | Apache License 2.0 |
| `raw/regexlib-manual-processed.sample.txt` | RegexLib-derived corpus, first 2000 lines (~500 pattern entries) of a larger file | [RegexLib Github](github.com/olivo/redos-detector), `regex_checker/rxxr/data/input/regexlib-manual-processed.txt`, a pre-processed research artifact from prior ReDoS-detection work (RXXR), not RegexLib.com directly | Source repo license applies; truncated here to a sample, not redistributed in full |

## Scope

Two rule files for Suricata, three for SpamAssassin, and a ~500-entry
prefix for RegexLib -- not the full ET Open ruleset (30,000+ rules) or
the full RegexLib-derived corpus (~12,000 lines / ~3000 entries in the
source file). Enough to validate the extraction/triage/benchmark
pipeline end to end with real data; scale up by re-running
`extract_dataset` against more files once the pipeline itself is
trusted.

## Pipeline

1. Extract + triage (append accepted patterns to data/corpus_patterns.txt) 

```bash
cargo run --example extract_dataset -- suricata <file>.rules [more files...]
cargo run --example extract_dataset -- spamassassin <file>.cf [more files...]
cargo run --example extract_dataset -- regexlib <file>.txt
```

2. Benchmark the 5 parsers against corpus
```bash
cargo bench --bench bench_dataset
```


## `corpus_patterns.txt`

Generated, not hand-maintained -- one pattern string per line (in
`pcre:"/.../flags"`-or-bare form, whatever `extract_dataset` pulled out),
already filtered to only the patterns `frontend::parse_pcre_rule`
accepts. Regenerate any time with the commands above after `rm
data/corpus_patterns.txt`.

## Triage results (this sample)

| Source | Extracted | Accepted | Backreference | Lookaround | Other |
|---|---|---|---|---|---|
| Suricata | 231 | 209 (90.5%) | 5 | 11 | 6 |
| SpamAssassin | 82 | 69 (84.1%) | 1 | 11 | 1 |
| RegexLib | 499 | 407 (81.6%) | 8 | 55 | 29 |
| **Total** | **812** | **685** | **14** | **77** | **36** |

(663 *unique* patterns land in `corpus_patterns.txt` after
cross-source deduplication -- some rules across files/sources are
identical or near-identical.)

Consistent with expectations discussed in `docs/MATCH_SEMANTICS.md`'s
neighbor doc and the earlier conversation this came out of: RegexLib
(general-purpose, crowd-sourced patterns -- password validators, email/
URL validators) has the highest lookaround/backreference rate;
Suricata/SpamAssassin (security-signature-oriented) the lowest. All
three converge on 80-90% acceptance -- the same subset RE2/`rust-regex`
would also accept, per `docs/EXTERNAL_ENGINES.md`.

## Finding

An early version of the bench swept the *entire* corpus per Criterion
iteration. `deriv_rec`/`deriv_loop` alone took ~55-60 **seconds** for one
such sweep against a single short probe string -- several orders of
magnitude past `deriv_bc`/`pderiv_bc`/`pderiv_standard` on the same
sweep. Root cause: `deriv_rec`/`deriv_loop` never call `simplify`
between derivative steps (a deliberate, pre-existing trade-off), so
their expression size grows unchecked across steps; combined with the
unanchored-search wildcard padding (`docs/MATCH_SEMANTICS.md`) real
dataset patterns get by default, and some already-large real patterns,
the cost compounds in a way the small synthetic patterns in
`bench_posix.rs` never trigger. `bench_dataset.rs`'s scope (bounded
corpus samples, not the full corpus, plus one group -
`dataset_worst_case_probe` - specifically isolating this gap) reflects
that finding directly; see that file's module doc comment for the full
account.
