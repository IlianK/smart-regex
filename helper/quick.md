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



# STACK DEPTH
cargo build --release --example bench_stack_depth_worker
cargo bench --bench bench_stack_depth



# PARSER AGREEMENT
cargo build --release --example demo_parser_agreement_worker
cargo run --release --example demo_parser_agreement
BENCH_MEM_CAP_MB=4096 cargo run --release --example demo_parser_agreement_pathological



# REPRESENTATIVE PATHOLOGICAL CASES: TIMING

# P60 (E): (a|b|ab)*, 'a'-repeated.
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,5000,20000,100000,300000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological_time

# P61 (E): (a|b|ab)*, 'ab'-repeated.
PATHO_ROW_ID=P61 PATHO_N=5,10,15,100,1000,10000,100000,500000,2500000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological_time

# P20 (E): (a|b|ab)+.
PATHO_ROW_ID=P20 PATHO_N=5,10,13,16,100,1000,10000,100000,300000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7C \
  cargo bench --bench bench_pathological_time

# P01 (A): (a+)+.
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,50,100,500,1000,2000,5000,10000,30000,50000,200000,1000000 \
  BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_time

# P09 (B): (a|aa)+.
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100,1000,5000,10000,50000,100000,500000,1000000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_time

# P25 (F): (a|b)*c.
PATHO_ROW_ID=P25 PATHO_N=1,3,7,10,50,300,1000,2000,3500,5000,10000,30000,50000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F25 \
  cargo bench --bench bench_pathological_time

# P27 (F): ([ab]+)*c.
PATHO_ROW_ID=P27 PATHO_N=1,3,7,10,15,20,25,30,40,50,100,1000,10000,30000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F27 \
  cargo bench --bench bench_pathological_time

# P28 (F): ([ab]*)*c.
PATHO_ROW_ID=P28 PATHO_N=1,3,7,10,15,20,25,30,40,50,100,1000,10000,30000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F28 \
  cargo bench --bench bench_pathological_time



# REPRESENTATIVE PATHOLOGICAL CASES: MEMORY

# P60 (E): (a|b|ab)*, 'a'-repeated.
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,5000,20000,100000,300000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological_memory

# P61 (E): (a|b|ab)*, 'ab'-repeated.
PATHO_ROW_ID=P61 PATHO_N=5,10,15,100,1000,10000,100000,500000,2500000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological_memory

# P20 (E): (a|b|ab)+.
PATHO_ROW_ID=P20 PATHO_N=5,10,13,16,100,1000,10000,100000,300000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7C \
  cargo bench --bench bench_pathological_memory

# P01 (A): (a+)+.
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,50,100,500,1000,2000,5000,10000,30000,50000,200000,1000000 \
  BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_memory

# P09 (B): (a|aa)+.
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100,1000,5000,10000,50000,100000,500000,1000000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_memory

# P25 (F): (a|b)*c.
PATHO_ROW_ID=P25 PATHO_N=1,3,7,10,50,300,1000,2000,3500,5000,10000,30000,50000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F25 \
  cargo bench --bench bench_pathological_memory

# P27 (F): ([ab]+)*c.
PATHO_ROW_ID=P27 PATHO_N=1,3,7,10,15,20,25,30,40,50,100,1000,10000,30000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F27 \
  cargo bench --bench bench_pathological_memory

# P28 (F): ([ab]*)*c.
PATHO_ROW_ID=P28 PATHO_N=1,3,7,10,15,20,25,30,40,50,100,1000,10000,30000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F28 \
  cargo bench --bench bench_pathological_memory
```