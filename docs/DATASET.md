# Datasets



## Sources

Three rule corpora, one per source.
Each entry below lists what the source looks like, a few real sample
patterns, and where to download the full corpus.

Few sample files picked from each original corpus are saved under
`data/raw/_Samples/`, one set per source, used by
`demo_frontend_categories` to demonstrate behaviours on real patterns.
Not part of the pipeline; `dataset_prepare` and `dataset_stats` read the
sources' own directories, not `_Samples`.


### Suricata/Snort

Exploit-signature style: hex-escaped literals (`\x3a` for `:`, `\x28`
for `(`), case-insensitive flags, alternation over obfuscation variants.

```
/Authorization\x3a\s*Basic\s*[a-zA-Z0-9]{255,}==/i
/clsid\s*\x3a\s*\x7B?\s*f6d90f11-9c73-11d3-b32e-00c04f990bb4/si
/eval\x28(String\x2EfromCharCode\x28|[a-z,0-9]{1,20}\x28String\x2EfromCharCode\x28)/i
```

[Proofpoint Emerging Threats Open](https://rules.emergingthreats.net/).

```bash
curl -O https://rules.emergingthreats.net/open/snort-2.9.7.0/emerging.rules.tar.gz
tar xzf emerging.rules.tar.gz -C data/raw/et-full
# -> data/raw/et-full/rules/*.rules
```

Only the `snort-*` tier is downloadable without an account.
`extract_suricata` reads generic `pcre:"..."` / `content:"..."` fields,
so the `snort-*` tier is what this pipeline uses.

Per the ruleset's own header:
SIDs `1–3464` and `100000000–100000908` are GPLv2; SIDs `2000000–2799999`
are BSD-3-clause.


### SpamAssassin

Keyword and evasion detection, plus structural header validation:
drug-name matching, the letter-gap trick spammers use to dodge keyword
filters, and a MIME boundary/UUID format check.

```
/xan[ae]x/i
/l.{0,2}e.{0,2}v.{0,2}i.{0,2}t.{0,2}r.{0,2}a/i
/boundary="[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}"/
```

[github.com/apache/spamassassin](https://github.com/apache/spamassassin/tree/trunk/rules), `rules/` of `trunk`.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/apache/spamassassin data/raw/spamassassin-full
# -> data/raw/spamassassin-full/rules/*.cf
```

Apache-2.0.
The source tarball on spamassassin.apache.org lacks the scored ruleset;
only the git repo has it.


### RegexLib

General-purpose validators, not security rules: email address, US phone
number, currency amount.

```
^[\w\.=-]+@[\w\.-]+\.[\w]{2,3}$
((\(\d{3}\) ?)|(\d{3}-))?\d{3}-\d{4}
^\$?\d{1,3}(,?\d{3})*(\.\d{1,2})?$
```

[github.com/superhuman/rxxr2](https://github.com/superhuman/rxxr2), a mirror of an academic ReDoS tool that scraped regexlib.com itself.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/superhuman/rxxr2 data/raw/rxxr2-source
# -> data/raw/rxxr2-source/data/input/regexlib-manual-processed.txt
```

MIT-licensed ("Copyright (c) 2016 University of Birmingham").




## Dataset statistics

To inspect the raw datasets with extraction counts, the accepted /
rejected split, rejection causes, and anchor-shape distribution, run:

```bash
# No args: reads the default sources under data/raw/
cargo run --release --example dataset_stats

# --verbose: list each rejected pattern next to its cause
cargo run --release --example dataset_stats -- --verbose

# --source: restrict the report to one or more sources
cargo run --release --example dataset_stats -- --source s2-elab

# stats for all five sources in one combined report
cargo run --release --example dataset_stats -- \
  --source suricata --source spamassassin --source regexlib --source s2-elab --source innovatorved
```

Sample size:

```
wc -l data/processed/*/prepared.jsonl
```

Run `dataset_stats` (all 5 sources by default):

| Table | Shows |
|---|---|
| Dataset coverage | per source: files, extracted, accepted / rejected, each with % |
| Pattern duplication | per source: of extracted and of accepted, how many are byte-identical to an earlier pattern's text, each with % |
| Accepted by match-intent reliability | `Direct` vs `Explicit ^...$` counts, each with % |
| Rejection reasons | per source: one column per cause (backreference, lookaround, word-boundary, anchor, flag-m, flag-x, other) |
| Anchor shape | per source: one column per shape (unparseable, none, fully-wrapped, start-only, end-only, nested) |
| PCRE flag distribution (all / accepted) | per flag: per-source count, total, effect |

Flags: `--source S` restrict to one source, `--verbose` list each
rejected pattern next to its cause, `--no-flags` suppress the two flag
tables.




## Dataset preparation

No input is generated, derived, or heuristically assembled, and nothing
is independently re-checked against the real parser either. `run_pipeline`
is **S2ELab-only**: it extracts, translates, and writes every usable
pattern's own given `matches`/`non_matches` as-is -- every one of them,
not a capped subset -- trusting the dataset author's own label
(`expected_match` on each `PreparedCase`). Suricata, SpamAssassin,
RegexLib, and Innovatorved still extract and translate their rules the
same as before (`dataset_stats`'s raw-corpus diagnostics use all five),
but `run_pipeline` itself never reads them.

Name the input files; there is no source or count argument:

```bash
cargo run --release --example dataset_prepare -- data/raw/s2-elab/RegexEval.json
```

Each run writes to `data/processed/s2-elab/prepared.jsonl`, replacing
the file rather than appending. Each *usable* pattern (at least one
given match and at least one given non-match) contributes every one of
its given examples -- a pattern with 10 given matches and 12 given
non-matches contributes 22 cases, not a fixed count. A pattern with only
one side, or neither, contributes nothing at all, and is counted
separately as unusable — see the summary `dataset_prepare` prints.

A rule whose raw pattern text is byte-identical to an earlier rule's in
the same run is skipped before translation even starts, rather than
writing a second, independent input group for the same regex —
doing so would silently overweight that one pattern in any benchmark
that reads `prepared.jsonl` back, in proportion to how many times the
source happens to repeat it. Counted separately as
`patterns_duplicate` in the summary and in
`reports/data/dataset_prepare.csv`, not folded into the unusable count.
The "Pattern duplication" table in `dataset_stats` reports the same
thing on the raw corpus.

The same run's summary numbers are *appended* as one row to
`reports/data/dataset_prepare.csv`, not replaced, so re-runs (or runs
across code changes) accumulate rather than overwrite each other's row.
The CSV's columns changed with this version (no `source`, a renamed
`patterns_duplicate`, nothing about verification); archive or clear the
old file before comparing across the change if that matters.





## Pipeline

1. `extract::extract_s2_elab` — one raw file's JSON text to
   `ExtractedRule`s: pattern plus the rule author's own
   `matches`/`non_matches`.
2. Dedup by raw pattern text against every rule already seen earlier in
   this run. A repeat is counted (`patterns_duplicate`) and skipped
   outright -- its fate is already decided by the first occurrence.
3. `prepare::core_regex_tagged` — parse and translate, with two size
   caps and one narrow anchor recovery. Rejected patterns stop here.
4. A small dedup/positive-wins helper in `mod.rs` turns
   `matches`/`non_matches` into `(text, is_match)` pairs, unchanged,
   uncapped, and unsampled.
5. Each pair becomes a `PreparedCase` directly -- no verification --
   written as JSON lines tagged with their `MatchIntent`.
6. `run_pipeline` — drives 1–5; returns `PipelineReport`.
   `load_prepared_cases` reads the result back for benchmarks.





## Match-intent

A pattern is accepted in one of two ways, recorded on every `PreparedCase`:

| `MatchIntent` | Pattern | Why |
|---|---|---|
| `ExplicitFullMatch` | Wrapped in `^...$` at its own top level | Anchors are provably redundant under full-string matching, so `core_regex_tagged` strips them before translation instead of rejecting |
| `UnanchoredAmbiguous` | No anchor anywhere | Benchmarked as a full match — this project's choice, not the source tool's native convention |

`ExplicitFullMatch` is narrow on purpose: a lone `^` or `$`, or one
nested in an `Or` branch or under a quantifier, is left untouched and
rejected by `translate`. `UnanchoredAmbiguous` covers the rest:
Suricata and SpamAssassin search a buffer by default, and RegexLib's
unanchored entries mix forgotten anchors (full-match intended) with
deliberate substring checks.

**Default scope.** The benches (`bench_corpus_time`, `bench_memory`,
`bench_translate`) default `BENCH_MATCH_INTENT` to `explicit` --
`ExplicitFullMatch` only. `UnanchoredAmbiguous` is opt-in via
`BENCH_MATCH_INTENT=direct` (that side alone) or `=both`. The reasoning:
an unanchored pattern's intended reading is genuinely ambiguous (search
vs. full match), so benchmarking it by default overstates how settled
that reading is; `ExplicitFullMatch` carries no such ambiguity.

**Positives vs negatives, for a given-input source.** A positive is
sound under either reading: a string in the pattern's own language is
trivially found by a search over that string. A negative is not
automatically so for an `UnanchoredAmbiguous` pattern: a string that
fails under full-match semantics can still contain a valid match
earlier in the same string under search semantics. `run_pipeline` does
not check for this -- every given negative is written as-is, trusting
the dataset's own label, for every match intent. This caveat only
applies to `UnanchoredAmbiguous` negatives; `ExplicitFullMatch` ones
have no such ambiguity, since full-match is the only reading there is.

**Benchmark cap.** `bench_corpus_time`/`bench_memory`/`bench_translate`
and `demo_parser_agreement` admit every distinct pattern per source by
default; `BENCH_PATTERN_LIMIT=N` caps that to the first N. The cap is on
distinct patterns, not on rows, so the number of patterns a bench
measures is predictable regardless of how many given inputs each one
carries.


