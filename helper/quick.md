```bash
# DATASET
cargo run --release --example dataset_stats
cargo run --release --example dataset_prepare -- data/raw/s2-elab/RegexEval.json
cargo run --release --example pathological_stats


# EXAMPLES
cargo run --example demo_match
cargo run --example demo_parse -- --diag 3 --parser deriv_std_rec
cargo run --example demo_parse -- --diag 3 --parser deriv_std_loop
cargo run --example demo_parse -- --diag 3 --parser deriv_bc
cargo run --example demo_parse -- --diag 3 --parser pderiv_std
cargo run --example demo_parse -- --diag 3 --parser pderiv_bc
cargo run --example demo_parse -- --parser all


# TRANSLATE
cargo run --release --example demo_translate -- data/raw/s2-elab/RegexEval.json --diag 1
cargo run --release --example demo_translate_reject
cargo run --release --example demo_translate_categories
cargo run --release --example demo_translate_sizes


# GROWTH
cargo run --release --example demo_simp_branches
cargo run --release --example demo_growth_deriv
cargo run --release --example demo_growth_pderiv
cargo run --release --example demo_growth_eval


# BENCHMARKS: S2ELAB
cargo build --release --example bench_worker

# Time: full corpus, split by outcome.
BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=CT1 \
  cargo bench --bench bench_corpus_time

# Memory: full corpus.
BENCH_RUN_LABEL=CM1 cargo bench --bench bench_corpus_memory

# Translate: full corpus.
BENCH_RUN_LABEL=CX1 cargo bench --bench bench_corpus_translate


# BENCHMARKS: PATHOLOGICAL

# Time: full sweep; memory cap protects large generator cases.
BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=PT1 \
  cargo bench --bench bench_pathological_time

# Memory: full sweep.
BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=PM1 \
  cargo bench --bench bench_pathological_memory

# Translate: full sweep.
BENCH_RUN_LABEL=PX1 cargo bench --bench bench_pathological_translate

# Stack depth: default sweep.
cargo bench --bench bench_stack_depth


# PARSER AGREEMENT
cargo build --release --example demo_parser_agreement_worker
cargo run --release --example demo_parser_agreement
cargo run --release --example demo_parser_agreement_pathological
# Full pathological sweep; may take 30+ minutes.


# REPRESENTATIVE PATHOLOGICAL CASES

# Confirmed measurable points plus large-scale targets.
# P60: (a|b|ab)*, repeated 'a'.
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological_time

# P61: (a|b|ab)*, repeated 'ab'.
PATHO_ROW_ID=P61 PATHO_N=5,10,15,500000,2500000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological_time

# P20: related + variant.
PATHO_ROW_ID=P20 PATHO_N=5,10,13,16,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7C \
  cargo bench --bench bench_pathological_time

# P01: (a+)+.
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,10000,30000,50000,200000,1000000 \
  BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_time

# P09: nested quantifiers, with more headroom.
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100000,500000,1000000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_time

# P25/P27/P28: alternation-star + literal; P27/P28 grow much faster.
PATHO_ROW_ID=P25,P27,P28 PATHO_N=1,3,7,10,50,300,1000,5000,10000,30000,50000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F \
  cargo bench --bench bench_pathological_time
```
