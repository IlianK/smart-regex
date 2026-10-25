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



# SINGLE PATTERNS
cargo bench --bench bench_pathological_time
PATHO_CATEGORY=A,B cargo bench --bench bench_pathological_time

# One pattern's own scaling curve (a Fig.-7-style line-chart source):
PATHO_ROW_ID=P01 PATHO_N=20,50,100,200,500,1000 cargo bench --bench bench_pathological_time

# The literal FLOPS'14 Fig. 7 reproduction (rows P60/P61, see below):
PATHO_ROW_ID=P60 PATHO_N=1000000,5000000,10000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_RUN_LABEL=FIG7A cargo bench --bench bench_pathological_time
PATHO_ROW_ID=P61 PATHO_N=500000,2500000,5000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_RUN_LABEL=FIG7B cargo bench --bench bench_pathological_time

# P20: same family as P60/P61 ((a|b|ab)+, no dedicated sweep in the
# dataset itself -- see P61's own notes), confirmed at n up to 100000
# to behave like P60/P61 (deriv_bc/pderiv_std both still well under
# 1.5s there); pushed here to the same paper scale as P60 for the same
# full Fig. 7 story with the + variant:
PATHO_ROW_ID=P20 PATHO_N=1000000,5000000,10000000 BENCH_ENTRY_TIMEOUT_MS=120000 \
  BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG7C cargo bench --bench bench_pathological_time

# P01: the single most-cited ReDoS example, (a+)+. deriv_std is dead by
# n~=25-30 already (see data/pathological/pathological_patterns.json's
# own category A suggested_n_notes); deriv_bc measured alive at n=10000
# (~1s) and gone by n=50000 in informal testing -- this sweep brackets
# that collapse point and pushes pderiv_bc/pderiv_std's own tail out to
# confirm they stay linear well past it:
PATHO_ROW_ID=P01 PATHO_N=10000,30000,50000,200000,1000000 \
  BENCH_ENTRY_TIMEOUT_MS=60000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_A \
  cargo bench --bench bench_pathological_time

# P09: same nested-quantifier family as P01, but deriv_bc measured
# alive and under 1s even at n=100000 in informal testing -- pushed
# further to find where it actually gives way, if it does before P60-
# style paper scale:
PATHO_ROW_ID=P09 PATHO_N=100000,500000,1000000,5000000 \
  BENCH_ENTRY_TIMEOUT_MS=120000 BENCH_MEM_CAP_MB=8192 BENCH_RUN_LABEL=FIG_B \
  cargo bench --bench bench_pathological_time

# P25/P27/P28: unlike P60/P61/P20, here deriv_bc itself (not just
# deriv_std) eventually times out -- informally measured alive at
# n=1000-10000 depending on the row and gone by n=10000-50000, while
# pderiv_bc/pderiv_std stay under 2s even at n=100000. This sweep
# brackets deriv_bc's own collapse for all three rows together (same
# category F mechanism, run as one comparison):
PATHO_ROW_ID=P25,P27,P28 PATHO_N=5000,10000,30000,50000,200000 \
  BENCH_ENTRY_TIMEOUT_MS=30000 BENCH_MEM_CAP_MB=4096 BENCH_RUN_LABEL=FIG_F \
  cargo bench --bench bench_pathological_time

```