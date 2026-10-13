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

Run `dataset_stats` (Suricata + SpamAssassin by default):

| Table | Shows |
|---|---|
| Dataset coverage | per source: files, extracted, accepted / rejected, each with % |
| Accepted by match-intent reliability | `Direct` vs `Explicit ^...$` counts, each with % |
| Rejection reasons | per source: one column per cause (backreference, lookaround, word-boundary, anchor, flag-m, flag-x, other) |
| Anchor shape | per source: one column per shape (unparseable, none, fully-wrapped, start-only, end-only, nested) |
| PCRE flag distribution (all / accepted) | per flag: per-source count, total, effect |

Flags: `--source S` restrict to one source, `--verbose` list each
rejected pattern next to its cause, `--no-flags` suppress the two flag
tables.




## Dataset preparation

To prepare all default datasets (Snort and SpamAssassin) in one go:

```bash
cargo run --release --example dataset_prepare
```

To prepare a specific dataset, name the source and list its input files with requested number of inputs to be generated / extracted (if dataset provides input). **Note: input N must be positive even integer**
(N=1 is not accepted, the smallest meaningful value is 2). 
There is no default it is mandatory and must be the first argument:

```bash
# Default sources, N=2 per pattern (1 match + 1 non-match)
cargo run --release --example dataset_prepare -- 2 suricata      data/raw/snort/*.rules 
cargo run --release --example dataset_prepare -- 2 spamassassin  data/raw/spamAssassin/*.cf 
cargo run --release --example dataset_prepare -- 2 regexlib      data/raw/regexLib/*.txt 
cargo run --release --example dataset_prepare -- 2 s2-elab       data/raw/s2-elab/RegexEval.json 
cargo run --release --example dataset_prepare -- 2 innovatorved  data/raw/innovatorved/data.json 

# Explicit source, N=10
cargo run --release --example dataset_prepare -- 10 snort data/raw/snort/*.rules

# Explicit source, N=20
cargo run --release --example dataset_prepare -- 20 spamassassin data/raw/spamAssassin/*.cf
```

`regexlib`, `s2-elab`, and `innovatorved` are opt-in: they are not
read by the default and must be named explicitly.

Each run writes to `data/processed/<source>/prepared.jsonl`, replacing
the file rather than appending. Each *usable* pattern contributes
exactly `<N>` verified cases: N/2 matches and N/2 non-matches, never
more, never fewer. A pattern that cannot reach N/2 of both (even after
retrying generation) contributes nothing at all and is counted
separately as below-target — see the summary `dataset_prepare` prints.

Unlike `prepared.jsonl`, the same run's summary numbers are *appended*
as one row to `reports/data/dataset_prepare.csv`, not replaced: the
point of that file is comparing how the pipeline behaves across
different `N` (or, over time, across code changes) for the same
source, so an earlier run's row needs to still be there once a later
one adds its own.





## Pipeline

1. `extract` — one raw file's text to `ExtractedRule`s: pattern plus
   source-specific context (`content:` fields for Suricata, rule kind
   for SpamAssassin, description for RegexLib).
2. `prepare::core_regex_tagged` — parse and translate, with two size
   caps and one narrow anchor recovery.
   Rejected patterns stop here.
3. `generate` — one accepted pattern's `Regex` to `Candidate` inputs,
   sampled structurally or from real `content:` fields / a local message
   corpus. Nothing trusted yet.
4. `prepare::verify_all` — check every candidate against the real
   parser, discard what doesn't hold, write survivors as
   `PreparedCase` JSON lines tagged with their `MatchIntent`.
5. `run_pipeline` — drives 1–4 for one source; returns `PipelineReport`.
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

**Positives vs negatives.** A positive is sound under either reading: a
string in the pattern's own language is trivially found by a search over
that string. A negative is not — `sample_negative_late` corrupts a
positive's *last* character, which can leave an earlier shorter match
untouched. `"aaa"` corrupted to `"aa☃"` is a full-match negative for
`a+`, but a false negative under search semantics, since `"aa"` alone
still matches. This holds only for `UnanchoredAmbiguous` negatives, not
for `ExplicitFullMatch` ones.

**Benchmark cap.** `bench_dataset.rs` and `demo_parser_agreement` admit
every distinct pattern per source by default; `BENCH_PATTERN_LIMIT=N`
caps that to the first N. The cap is on distinct patterns, not on rows,
so the number of patterns a bench measures is predictable regardless of
how many verified inputs each one carries.


