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
(PLDI 2024) paper draws from (Snort/Suricata, SpamAssassin, RegexLib). The paper itself provides no public artifact/dataset, so these are
independently sourced.



## Sources

| Directory/file | Source | Fetched from | License / terms |
|---|---|---|---|
| `raw/emerging-exploit.rules`, `raw/emerging-web_client.rules` | Suricata/Snort `pcre:` rules, Emerging Threats OPEN ruleset | [Suratica Github](github.com/seanlinmt/suricata) (mirror of the ET Open ruleset) | ET Open rules are freely distributable for IDS use; see [rules.emergingthreats.net](https://rules.emergingthreats.net/OPEN_download_instructions.html) for the canonical source and current terms |
| `raw/20_drugs.cf`, `raw/20_body_tests.cf`, `raw/20_head_tests.cf` | SpamAssassin default rules | [SpamAssassin Github](github.com/apache/spamassassin) (official read-only mirror), `rules/` dir | Apache License 2.0 |
| `raw/regexlib-manual-processed.sample.txt` | RegexLib-derived corpus, first 2000 lines (~500 pattern entries) of a larger file | [RegexLib Github](github.com/olivo/redos-detector), `regex_checker/rxxr/data/input/regexlib-manual-processed.txt`, a pre-processed research artifact from prior ReDoS-detection work (RXXR), not RegexLib.com directly | Source repo license applies; truncated here to a sample, not redistributed in full |

