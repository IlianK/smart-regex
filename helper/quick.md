```bash
# DATASET
cargo run --release --example dataset_stats
cargo run --release --example dataset_prepare -- data/raw/s2-elab/RegexEval.json
cargo run --release --example pathological_stats


# DATASET EVIDENCE
cargo run --release --example dataset_skew_check -- data/raw/s2-elab/RegexEval.json
cargo run --release --example demo_bound_usage


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
# One worker, 12 GiB cap, everywhere below (time and memory, S2ELab
# and pathological alike). Real corpus patterns with nested optional
# groups (e.g. the S2ELab loose-formatting family) blow deriv_std_rec's
# memory up almost immediately, even on short inputs -- sharing a
# smaller cap across several concurrent workers turned too many of
# those into false crashes. Single-worker is slower (no concurrency on
# the memory benches; a run can take 8h+), accepted here since this is
# an exploratory run on experiment_run, not the defended thesis scope.
cargo build --release --example bench_worker

# Time
BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=12288 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=CT1 \
  cargo bench --bench bench_corpus_time

# Memory
BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=CM1 \
  cargo bench --bench bench_corpus_memory

# Translate
BENCH_RUN_LABEL=CX1 cargo bench --bench bench_corpus_translate



# BENCHMARKS: PATHOLOGICAL

# Time
BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=12288 BENCH_RUN_LABEL=PT1 \
  cargo bench --bench bench_pathological_time

# Memory
BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=PM1 \
  cargo bench --bench bench_pathological_memory

# Translate
BENCH_RUN_LABEL=PX1 cargo bench --bench bench_pathological_translate



# STACK DEPTH 
cargo build --release --example bench_stack_depth_worker
STACKDEPTH_N_CAP=4294967295 cargo bench --bench bench_stack_depth



# PARSER AGREEMENT
cargo build --release --example demo_parser_agreement_worker
cargo run --release --example demo_parser_agreement
BENCH_MEM_CAP_MB=12288 cargo run --release --example demo_parser_agreement_pathological



# ENCODE / DERIVE / DECODE: PATTERN-SIZE SWEEP (E37, E38)
cargo build --release --example bench_worker

# E37
CORE_REGEX_MAX_NODES=200000 BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 \
  PATHO_ROW_ID=E37 PATHO_P=100,500,1000,2000,5000,10000 BENCH_RUN_LABEL=PHASE_E37_WIDE \
  cargo bench --bench bench_pathological_time

# E38
CORE_REGEX_MAX_NODES=200000 BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 \
  PATHO_ROW_ID=E38 PATHO_P=100,500,1000,2000,5000,10000 BENCH_RUN_LABEL=PHASE_E38_WIDE \
  cargo bench --bench bench_pathological_time


# ENCODE / DERIVE / DECODE: PATTERN-SIZE SWEEP MEMORY (E37, E38)
cargo build --release --example bench_worker

# E37
CORE_REGEX_MAX_NODES=200000 BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 \
  PATHO_ROW_ID=E37 PATHO_P=100,500,1000,2000,5000,10000 BENCH_RUN_LABEL=PHASE_E37_WIDE \
  cargo bench --bench bench_pathological_memory

# E38
CORE_REGEX_MAX_NODES=200000 BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 \
  PATHO_ROW_ID=E38 PATHO_P=100,500,1000,2000,5000,10000 BENCH_RUN_LABEL=PHASE_E38_WIDE \
  cargo bench --bench bench_pathological_memory


# REPRESENTATIVE PATHOLOGICAL CASES: TIMING

# P60
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,5000,20000,100000,300000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological_time

# P61
PATHO_ROW_ID=P61 PATHO_N=5,10,15,100,1000,10000,100000,500000,2500000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological_time

# P01
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,50,100,500,1000,2000,5000,10000,30000,50000,200000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_time

# P09
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100,1000,5000,10000,50000,100000,500000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_time



# REPRESENTATIVE PATHOLOGICAL CASES: MEMORY

# P60
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,5000,20000,100000,300000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological_memory

# P61
PATHO_ROW_ID=P61 PATHO_N=5,10,15,100,1000,10000,100000,500000,2500000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological_memory

# P01
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,50,100,500,1000,2000,5000,10000,30000,50000,200000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_memory

# P09
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100,1000,5000,10000,50000,100000,500000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_memory



# POST-RUN: WORST-OFFENDER PATTERNS

cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_corpus_time_CT1_entries.csv micros 5
cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_corpus_memory_CM1_entries.csv peak_bytes 5
cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_pathological_time_PT1_entries.csv micros 5
cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_pathological_memory_PM1_entries.csv peak_bytes 5
```