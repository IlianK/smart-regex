# Datasets

## Dataset Sources

### Samples

Few sample files picked from original corpus saved under `data/raw/_Samples/`,
one set per source, used by `demo_frontend_categories` to demonstrate
behaviours on real patterns. Not part of the pipeline; `dataset_prepare`
and `dataset_stats` read the sources' own directories, not `_Samples`.

**Suricata/Snort**: Exploit-signature style: hex-escaped literals (`\x3a`
for `:`, `\x28` for `(`), case-insensitive flags, alternation over
obfuscation variants.

```
/Authorization\x3a\s*Basic\s*[a-zA-Z0-9]{255,}==/i
/clsid\s*\x3a\s*\x7B?\s*f6d90f11-9c73-11d3-b32e-00c04f990bb4/si
/eval\x28(String\x2EfromCharCode\x28|[a-z,0-9]{1,20}\x28String\x2EfromCharCode\x28)/i
```

**SpamAssassin**: Keyword and evasion detection, plus structural header
validation: simple drug-name matching, the letter-gap trick spammers use
to dodge keyword filters, and a MIME boundary/UUID format check.

```
/xan[ae]x/i
/l.{0,2}e.{0,2}v.{0,2}i.{0,2}t.{0,2}r.{0,2}a/i
/boundary="[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}"/
```

**RegexLib**: General-purpose validators, not security rules: email
address, US phone number, currency amount.

```
^[\w\.=-]+@[\w\.-]+\.[\w]{2,3}$
((\(\d{3}\) ?)|(\d{3}-))?\d{3}-\d{4}
^\$?\d{1,3}(,?\d{3})*(\.\d{1,2})?$
```


### Full corpus

How to download the full corpus per source, and where to put it:

**Snort/Suricata** — [Proofpoint Emerging Threats Open](https://rules.emergingthreats.net/).

```bash
curl -O https://rules.emergingthreats.net/open/snort-2.9.7.0/emerging.rules.tar.gz
tar xzf emerging.rules.tar.gz -C data/raw/et-full
# -> data/raw/et-full/rules/*.rules
```

Only the `snort-*` tier is downloadable without an account; the
`suricata-*` tier is not open. `extract_suricata` reads generic
`pcre:"..."` / `content:"..."` fields and works with either, so the
`snort-*` tier is what this pipeline uses. 

Per the ruleset's own header:
SIDs `1–3464` and `100000000–100000908` are GPLv2; SIDs `2000000–2799999`
are BSD-3-clause.


**SpamAssassin** — [github.com/apache/spamassassin](https://github.com/apache/spamassassin/tree/trunk/rules), `rules/` of `trunk`.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/apache/spamassassin data/raw/spamassassin-full
# -> data/raw/spamassassin-full/rules/*.cf
```

The source tarball on spamassassin.apache.org does *not* include the
scored ruleset, only the git repo has it checked in. Apache-2.0.


**RegexLib** — [github.com/superhuman/rxxr2](https://github.com/superhuman/rxxr2), a mirror of an academic ReDoS tool that scraped regexlib.com itself.

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/superhuman/rxxr2 data/raw/rxxr2-source
# -> data/raw/rxxr2-source/data/input/regexlib-manual-processed.txt
```

MIT-licensed ("Copyright (c) 2016 University of Birmingham")


## Statistics

Run `dataset_stats` (all three sources at once by default) to see, per
source:

- extraction counts: rules read, patterns extracted
- acceptance: how many patterns `core_regex_tagged` accepts versus
  rejects -- the same function and the same safety caps
  `dataset_prepare` benchmarks against, not plain `parse_pcre_rule`
- match-intent reliability: of the accepted patterns, how many were a
  top-level `^...$` wrap (`ExplicitFullMatch`, stripped before
  translation since the anchors are then redundant) versus carried no
  anchor at all (`UnanchoredAmbiguous`) -- see "Match-intent reliability
  tiers" below
- rejection causes: backreference / lookaround / word-boundary / anchor
  (any placement other than a top-level `^...$` wrap) / flag-m / flag-x
  / other (includes a pattern too large for the pipeline's own depth/node
  caps)
- PCRE flag distribution: which flags appear in the corpus, and what
  each is treated as (honoured / ignored / rejected)

Useful flags: `--source S` restrict to one source, `--verbose` list each
rejected pattern next to its cause, `--no-flags` suppress the flag
tables.


## Preparing the dataset

```
cargo run --release --example dataset_prepare -- suricata data/raw/snort/*.rules
cargo run --release --example dataset_prepare -- spamassassin data/raw/spamAssassin/*.cf
cargo run --release --example dataset_prepare -- regexlib data/raw/regexLib/*.txt
```

Each run writes to `data/processed/<source>/prepared.jsonl`, replacing
the file (not appending). Each pattern contributes up to `--variants N`
(default 5) independently verified positive inputs, plus up to `N`
negatives. Sample size:

```
wc -l data/processed/*/prepared.jsonl
```

## Pipeline

1. `extract`: Read one raw rule file's text and produce `ExtractedRule`
   values: the pattern string plus source-specific context (`content:`
   fields for Suricata, rule kind for SpamAssassin, description for
   RegexLib).
2. `prepare::core_regex_tagged`: Parse and translate the pattern,
   applying the same delimiter-stripping/flag-check/case-fold steps as
   `parse_pcre_rule`, plus two size caps and one anchor recovery (see
   "Match-intent reliability tiers"). Rejects the pattern here if none
   of that succeeds.
3. `generate`: Turn one accepted pattern's `Regex` into `Candidate`
   inputs, sampled either from the pattern's own structure or (where
   available) from real `content:` fields or a local message corpus.
   Nothing trusted yet.
4. `prepare::verify_all`: Verify every candidate against the real
   parser, discard what doesn't hold up, and write survivors as
   `PreparedCase` JSON lines, each tagged with the `MatchIntent` its
   pattern carried. Holds the remaining safety cap (`MAX_AREGEX_NODES`)
   that lets the pipeline run unattended over a full real corpus.
5. `mod::run_pipeline`: Drives 1 → 2 → 3 → 4 for one source and returns
   a `PipelineReport`; `mod::load_prepared_cases` reads the result back
   for benchmarks.


## Match-intent reliability tiers

Accepted patterns are not all reliable in the same way. A pattern
wrapped in `^...$` at its own top level is an explicit, unambiguous
statement from its own author that the whole pattern is the match, with
nothing before or after -- exactly what full-string matching already
checks, so the anchors are provably redundant and `core_regex_tagged`
strips them before translation rather than rejecting the pattern the
way every other anchor placement is rejected (see
[FRONTEND.md](FRONTEND.md)'s "Anchors are rejected, not padded"). This
recovery is narrow on purpose: a lone `^` or `$`, or one nested inside an
`Or` branch or under a quantifier, is not provably redundant (the rest of
the match may still need to stay open), so it is left untouched and
`translate` rejects it, same as before this recovery existed.

A pattern that carries no anchor at all is accepted too, and benchmarked
as a full-string match regardless -- but that reading is this project's
own interpretive choice, not a reproduction of the source tool's native
convention. Suricata's and SpamAssassin's own tools search a pattern
within a buffer by default when it carries no anchor; RegexLib's
unanchored entries are a genuine mix of validators that simply forgot
their own anchors (full-match intended) and deliberate substring checks
(search intended), and nothing in the pattern text alone tells the two
apart.

Every `PreparedCase` carries a `MatchIntent` recording which case
applied, so a benchmark or a thesis claim can scope itself to the
reliable tier alone when it needs to, rather than silently picking one
reading for every accepted pattern:

| `MatchIntent` | What it means | How it's produced |
|---|---|---|
| `ExplicitFullMatch` | Author's own explicit full-match statement | `^...$` wrap, stripped before translation |
| `UnanchoredAmbiguous` | This project's own full-match reading | No anchor found; translated as-is |

Positive candidates are sound under either reading regardless of tier:
if a string is in the pattern's own language exactly, it is trivially
also found by a search over that string (zero padding needed on either
side). Negative candidates are not: `generate::sample_negative_late`
corrupts a positive sample's *last* character, which can leave an
earlier, shorter match sitting in the string untouched -- a corrupted
`"aaa"` for pattern `a+` becomes `"aa☃"`, a genuine full-match negative
that is a false negative under the pattern's own native search reading,
since `"aa"` alone still matches. This is a property of negatives in the
`UnanchoredAmbiguous` tier specifically, not of the tier split itself.

Benchmarks (`bench_dataset.rs`, `bench_external.rs`,
`demo_parser_agreement`) take the first 30 *distinct patterns* per
source, not the first 30 rows, and then every verified input belonging
to those 30 (`BENCH_PATTERN_LIMIT` to change the cap). Rows per pattern
are uneven: one pattern can contribute up to `2 × --variants` rows
(positives and negatives together), while a short pattern like `a`
contributes 1. Capping on rows would cover only a handful of distinct
patterns; capping on patterns guarantees 30.
