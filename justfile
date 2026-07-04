# Task runner for the ecdsafail-challenge repo.
#   just            → list recipes
#   just analysis   → full scientific-rigor suite (replaces analysis/run.sh)
#   just kani       → Kani bit-precise proofs
#   just all        → build + score + depth + analysis
#
# Analysis stages run from the analysis/ directory (as the Python scripts expect).
#
# Python env: managed by uv (pyproject.toml + uv.lock, transitively hash-pinned).
# The reproducible way to run the suite is through the locked virtualenv:
#   uv sync --locked          # build .venv from uv.lock (z3, Python 3.11 floor)
#   uv run just analysis      # recipes' `python3` resolves to the locked venv
# `analysis/requirements.txt` is a pip fallback that mirrors uv.lock.
#
# PYTHON selects the interpreter for every python recipe (default `python3`, which
# under `uv run` is the locked venv). CI pins the 3.11 floor; to reproduce the
# pycheck syntax guard against 3.11 without uv:
#   just PYTHON=python3.11 pycheck
PYTHON := "python3"

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

# Full 15-stage analysis suite (formal proofs + physical cost model).
# `solinas-emitted` is a separate recipe (like `kani`): it is a real proof but ~5 min
# (256-bit emitted-gate replay), far heavier than the other z3 stages, so it is kept
# out of the default suite. Run it explicitly: `just solinas-emitted`.
analysis: toolkit solinas peephole mbuc refadders controlled-lookup lookup-cost completeness direct-lookup offset mid-ladder recover toyshor cost-model ecdlp

# Byte-compile all analysis python — catches version-incompatible syntax. To
# reproduce CI's 3.11-floor guard exactly, pass a 3.11 interpreter:
#   just PYTHON=python3.11 pycheck
pycheck:
    {{PYTHON}} -m compileall -q analysis

# Kani (bit-precise BMC) harnesses on the real Rust alloy U256 type.
kani:
    cd analysis && bash verify/run_kani.sh

# Reusable proof toolkit self-test: the generalized z3 sim.rs op-stream replayer
# that the mbuc proof (and future op-stream proofs) drive (ADR 0028/0029).
toolkit:
    @echo "### proof_toolkit self-test: symbolic sim.rs op-stream replayer (z3, ADR 0028/0029) ###"
    cd analysis/verify && {{PYTHON}} -m proof_toolkit.selftest

solinas:
    @echo "### Solinas modular-reduction proof — step-for-step model (z3) ###"
    cd analysis && {{PYTHON}} verify/solinas_reduction.py

# Solinas reduction proved over the REAL emitted mod_add_qq op-stream (ADR 0031, F2):
# the emitter-bound complement to `solinas` and the Kani twin. ~5 min (256-bit gate
# replay); not part of `just analysis`. Needs the emitted artifact + z3.
solinas-emitted:
    @echo "### Solinas reduction over the EMITTED mod_add_qq gates (z3, ADR 0031, F2; ~5 min) ###"
    cd analysis && {{PYTHON}} verify/solinas_reduction_emitted.py

peephole:
    @echo "### Peephole / adder / comparator proofs (z3) ###"
    cd analysis && {{PYTHON}} verify/peephole_identities.py

mbuc:
    @echo "### Emitted _fast adder measurement-based uncompute: HMR+cz_if phase proof (z3, F1/F2) ###"
    cd analysis && {{PYTHON}} verify/mbuc_phase_correction.py

refadders:
    @echo "### Reference kickmix adder validation (source-paper artifacts) ###"
    cd analysis && {{PYTHON}} verify/validate_reference_adders.py

controlled-lookup:
    @echo "### Constructed controlled table-lookup validation (ladder QROM primitive) ###"
    cd analysis && {{PYTHON}} verify/controlled_lookup.py

lookup-cost:
    @echo "### Windowed-lookup (QROM) cost: measured unary-iteration read (issue #4) ###"
    cd analysis && {{PYTHON}} verify/ladder_lookup_cost.py

completeness:
    @echo "### Empirical adder-completeness collision rate (issue #5, Path A) ###"
    cd analysis && {{PYTHON}} verify/completeness_collision_rate.py

direct-lookup:
    @echo "### Direct-lookup first window: circuit-level infinity-start removal (issue #5a) ###"
    cd analysis && {{PYTHON}} verify/direct_lookup_init.py

offset:
    @echo "### Offset window encoding: remove the zero-window infinity term (issue #5b) ###"
    cd analysis && {{PYTHON}} verify/offset_window_encoding.py

mid-ladder:
    @echo "### Exact end-to-end mid-ladder exceptional bound (issue #28) ###"
    cd analysis && {{PYTHON}} verify/mid_ladder_bound.py

recover:
    @echo "### End-to-end Shor-ECDLP discrete-log recovery on toy curves (issue #46) ###"
    cd analysis && {{PYTHON}} verify/shor_ecdlp_recovery.py

toyshor:
    @echo "### Gate-level QFT toy Shor-ECDLP capstone: unify the gate-level pieces (issue #55) ###"
    cd analysis && {{PYTHON}} verify/toy_shor_qft.py

cost-model:
    @echo "### Physical fault-tolerant cost model ###"
    cd analysis && {{PYTHON}} cost_model.py

ecdlp:
    @echo "### Derived + measured full-ECDLP cost (measured primitive x paper's ladder) ###"
    cd analysis && {{PYTHON}} ecdlp_estimate.py

# ── composite ───────────────────────────────────────────────────────────────

# Full pipeline: build the circuit, score it, measure depth, run the analysis suite.
all: build score depth analysis
