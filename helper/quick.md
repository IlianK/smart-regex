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
# One worker, 12 GiB cap, everywhere below (S2ELab and pathological
# alike). Real corpus patterns with nested optional groups (e.g. the
# S2ELab loose-formatting family) blow deriv_std_rec's memory up
# almost immediately, even on short inputs -- sharing a smaller cap
# across several concurrent workers turned too many of those into
# false crashes. Single-worker is slower (no concurrency; a run can
# take 8h+), accepted here since this is an exploratory run on
# experiment_run, not the defended thesis scope. bench_corpus/
# bench_pathological now measure time and memory together in one
# pass per entry (bench_worker already reports both), replacing what
# used to be a separate _time and _memory command for each.
cargo build --release --example bench_worker

# Time and memory, together
BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_SPLIT_OUTCOME=1 BENCH_RUN_LABEL=C1 \
  cargo bench --bench bench_corpus

# Translate
BENCH_RUN_LABEL=CX1 cargo bench --bench bench_corpus_translate



# BENCHMARKS: PATHOLOGICAL

# Time and memory, together
BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=P1 \
  cargo bench --bench bench_pathological

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

# E37: now reaches the same p-range as E38 instead of stopping at 2,000
CORE_REGEX_MAX_NODES=200000 BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 \
  PATHO_ROW_ID=E37 PATHO_P=100,500,1000,2000,5000,10000 BENCH_RUN_LABEL=PHASE_E37_WIDE \
  cargo bench --bench bench_pathological

# E38: same override — no longer stops at p=1,000
CORE_REGEX_MAX_NODES=200000 BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 \
  PATHO_ROW_ID=E38 PATHO_P=100,500,1000,2000,5000,10000 BENCH_RUN_LABEL=PHASE_E38_WIDE \
  cargo bench --bench bench_pathological


# REPRESENTATIVE PATHOLOGICAL CASES
# Same cap as every other command above; the 120s timeout is this
# section's own override, unrelated to the memory environment.

# P60
PATHO_ROW_ID=P60 PATHO_N=10,50,100,500,1000,5000,20000,100000,300000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG7A \
  cargo bench --bench bench_pathological

# P61
PATHO_ROW_ID=P61 PATHO_N=5,10,15,100,1000,10000,100000,500000,2500000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG7B \
  cargo bench --bench bench_pathological

# P01
PATHO_ROW_ID=P01 PATHO_N=5,10,15,20,50,100,500,1000,2000,5000,10000,30000,50000,200000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological

# P09
PATHO_ROW_ID=P09 PATHO_N=5,10,15,20,25,100,1000,5000,10000,50000,100000,500000,1000000,5000000,10000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=12288 BENCH_MEM_WORKERS=1 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological



# POST-RUN: WORST-OFFENDER PATTERNS
# Once a benchmark above has written its _entries.csv, find which
# (pattern, input) caused each parser's worst case by a chosen metric
# column, instead of grepping/sorting the CSV by hand. Re-run with a
# different entries CSV / metric / top-n as needed; these two files
# match the C1/P1 labels used above -- each already carries both the
# micros and peak_bytes columns, one pass having measured both.
cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_corpus_C1_entries.csv micros 5
cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_corpus_C1_entries.csv peak_bytes 5
cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_pathological_P1_entries.csv micros 5
cargo run --release --example demo_peak_offenders -- \
  reports/benches/bench_pathological_P1_entries.csv peak_bytes 5
```
