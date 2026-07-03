# Task runner for the ecdsafail-challenge repo.
#   just            → list recipes
#   just analysis   → full scientific-rigor suite (replaces analysis/run.sh)
#   just kani       → Kani bit-precise proofs
#   just all        → build + score + depth + analysis
#
# Analysis stages run from the analysis/ directory (as the Python scripts expect).

# List available recipes.
default:
    @just --list

# ── Rust circuit pipeline ───────────────────────────────────────────────────

# Build the scored circuit -> ops.bin.
build:
    cargo run --release --bin build_circuit

# Score the circuit over the harness shots -> score.json (benchmark.sh).
score:
    bash benchmark.sh

# Measure toffoli/gate depth of ops.bin -> depth.json.
depth:
    cargo run --release --bin depth_report

# Run the unit tests (heavy end-to-end probes are #[ignore]'d).
test:
    cargo test --release

# ── analysis: scientific-rigor suite (z3 proofs + cost model) ───────────────

# Full 11-stage analysis suite (formal proofs + physical cost model).
analysis: solinas peephole refadders controlled-lookup lookup-cost completeness direct-lookup offset mid-ladder cost-model ecdlp

# Kani (bit-precise BMC) harnesses on the real Rust alloy U256 type.
kani:
    cd analysis && bash verify/run_kani.sh

solinas:
    @echo "### Solinas modular-reduction proof (z3) ###"
    cd analysis && python3 verify/solinas_reduction.py

peephole:
    @echo "### Peephole / adder / comparator proofs (z3) ###"
    cd analysis && python3 verify/peephole_identities.py

refadders:
    @echo "### Reference kickmix adder validation (source-paper artifacts) ###"
    cd analysis && python3 verify/validate_reference_adders.py

controlled-lookup:
    @echo "### Constructed controlled table-lookup validation (ladder QROM primitive) ###"
    cd analysis && python3 verify/controlled_lookup.py

lookup-cost:
    @echo "### Windowed-lookup (QROM) cost: measured unary-iteration read (issue #4) ###"
    cd analysis && python3 verify/ladder_lookup_cost.py

completeness:
    @echo "### Empirical adder-completeness collision rate (issue #5, Path A) ###"
    cd analysis && python3 verify/completeness_collision_rate.py

direct-lookup:
    @echo "### Direct-lookup first window: circuit-level infinity-start removal (issue #5a) ###"
    cd analysis && python3 verify/direct_lookup_init.py

offset:
    @echo "### Offset window encoding: remove the zero-window infinity term (issue #5b) ###"
    cd analysis && python3 verify/offset_window_encoding.py

mid-ladder:
    @echo "### Exact end-to-end mid-ladder exceptional bound (issue #28) ###"
    cd analysis && python3 verify/mid_ladder_bound.py

cost-model:
    @echo "### Physical fault-tolerant cost model ###"
    cd analysis && python3 cost_model.py

ecdlp:
    @echo "### Derived + measured full-ECDLP cost (measured primitive x paper's ladder) ###"
    cd analysis && python3 ecdlp_estimate.py

# ── composite ───────────────────────────────────────────────────────────────

# Full pipeline: build the circuit, score it, measure depth, run the analysis suite.
all: build score depth analysis
