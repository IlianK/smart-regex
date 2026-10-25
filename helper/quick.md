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




# GROWTH DEMOS
cargo run --release --example demo_simp_branches

cargo run --release --example demo_growth_deriv

cargo run --release --example demo_growth_pderiv

cargo run --release --example demo_growth_eval




# BENCHES SE2LAB
cargo build --release --example bench_worker

# Time
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CT0 \
  cargo bench --bench bench_corpus_time

BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=CT1 \
  cargo bench --bench bench_corpus_time

# Memory
BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=1000 BENCH_RUN_LABEL=CM0 \
  cargo bench --bench bench_corpus_memory

BENCH_RUN_LABEL=CM1 cargo bench --bench bench_corpus_memory

# Translate
BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=CX0 \
  cargo bench --bench bench_corpus_translate

BENCH_RUN_LABEL=CX1 cargo bench --bench bench_corpus_translate




# BENCHES PATHO

# Time
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 \
  BENCH_RUN_LABEL=PT0 cargo bench --bench bench_pathological_time

BENCH_RUN_LABEL=PT1 cargo bench --bench bench_pathological_time

# Memory
PATHO_N=5 PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_ENTRY_TIMEOUT_MS=500 \
  BENCH_RUN_LABEL=PM0 cargo bench --bench bench_pathological_memory

BENCH_RUN_LABEL=PM1 cargo bench --bench bench_pathological_memory

# Translate
PATHO_P=5 BENCH_PATTERN_LIMIT=5 BENCH_RUN_LABEL=PX0 \
  cargo bench --bench bench_pathological_translate

BENCH_RUN_LABEL=PX1 cargo bench --bench bench_pathological_translate




# PARSER AGREEMENT
cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement

cargo build --release --example demo_parser_agreement_worker
cargo run   --release --example demo_parser_agreement_pathological


# Fig. 7(a) reproduction — only P60, at paper scale
PATHO_ROW_ID=P60 PATHO_N=1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 \
  BENCH_RUN_LABEL=FIG7A cargo bench --bench bench_pathological_time

# Fig. 7(b) reproduction — only P61, at paper scale
PATHO_ROW_ID=P61 PATHO_N=500000,2500000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 \
  BENCH_RUN_LABEL=FIG7B cargo bench --bench bench_pathological_time

```