# analysis/ — scientific-rigor layer

Turns the challenge circuit from a leaderboard number into a verifiable,
physically-grounded result. Lives outside `editablePaths` (`src/point_add`), so
nothing here can affect the circuit or the score.

| File | What it does |
|---|---|
| `verify/proof_toolkit/` | The verification **methodology** packaged as a reusable library (ADR 0028/0029): `symsim.py` is a faithful z3 model of every `src/sim.rs` op (CX/CCX/CZ/CCZ/HMR/R/Swap/condition-stack/…) that **replays an emitted op-stream** with measurement outcomes **free/∀**, plus the `{"widths":[…]}` dump loader (`mbuc_dump.rs` format) and z3 `prove`/`find`/teeth helpers. Generalized out of the ADR 0027 proof so the "prove-what-you-emit" pattern is reusable by future op-stream proofs; `mbuc_phase_correction.py` is its first consumer. `selftest.py` (`just toolkit`) pins each op's symbolic semantics against hand-computed truth (incl. an HMR+`cz_if` phase-cancellation teeth), independent of any dumped artifact. |
| `verify/solinas_reduction.py` | z3 proof: `mod_add_qq` computes `(acc+a) mod p` for **all** `acc,a ∈ [0,p)`, and its overflow ancilla uncomputes to \|0⟩. |
| `verify/peephole_identities.py` | z3 proofs of the constprop CCX identities, the ripple-carry adder recurrence, and the borrow-chain comparator (22 lemmas). |
| `verify/mbuc_phase_correction.py` | z3 proof of the **emitted `_fast` adder's measurement-based uncompute** — the HMR + `cz_if` phase correction the scored hot path runs but the plain-adder z3/Kani proofs never model (referee findings F1/F2, ADR 0027). Replays the **actually emitted** `cuccaro_add_fast` op-stream (dumped by the real `B` builder into `mbuc_fast_adder_ops.json` via `src/point_add/mbuc_dump.rs`; a `#[cfg(test)]` drift guard keeps the artifact byte-identical to a fresh emit) through a z3 model of `src/sim.rs`, with the measurement outcomes **free/∀** (not the random XOF), and proves — at widths 2..**256** — `acc'=(a+acc) mod 2^n`, `a`/`c_in`/carries clean, and **net phase 0 for every input and every measurement outcome** (the HMR kickback `carry·m` is exactly cancelled by `cz_if`'s `x·y·m`). A teeth check shows dropping the `cz_if` corrections makes the phase claim fail (sat). Drives the shared `verify/proof_toolkit/` replayer (ADR 0029). |
| `verify/run_kani.sh` | Runs the Kani (bit-precise BMC) harnesses in `src/kani_proofs.rs` that bind to the **real Rust `alloy` U256 type** (not an abstract model). |
| `verify/kickmix_sim.py` | Independent, spec-faithful simulator for kickmix `.kmx` circuits (the source paper's format) — re-derives the semantics `src/sim.rs` implements. |
| `verify/validate_reference_adders.py` | Fuzz-validates the **source paper's** reference in-place adders (`verify/reference_circuits/`, from arXiv:2603.28846v2) — correct output, clean/dirty ancilla restored, phase +1 — and confirms its three negative-control circuits are **rejected**. |
| `verify/controlled_lookup.py` | Constructs and validates a self-contained **controlled** table lookup `r0 ^= ctrl ? r2[r1] : 0` (the ladder's `3·2^w` QROM primitive), in both reversible and measurement-based-uncomputation forms — the reference `table_lookup_3x3.kmx` is only an illustrative extract (issue #3). |
| `verify/ladder_lookup_cost.py` | **Measures** the windowed-lookup (QROM) cost that was derived in the estimate (issue #4, ADR 0010). Builds an optimized **unary-iteration** table read `out ^= T[addr]` as a kickmix circuit in **both** uncompute forms — reversible and measurement-based (MBUC: `HMR` + `CZ`/`Z` phase fixup, issue #29) — validates each exhaustively (correct read, registers unchanged, ancilla cleared, phase `+1`; teeth check that deleting the MBUC fixups fails the phase test), and measures **`2^(w+1)−4` Toffoli reversible / `2^w−2` MBUC, `w` ancilla per read** — below the paper's `3·2^w`. |
| `verify/completeness_collision_rate.py` | **Measures** the affine adder's exceptional-input rate across a faithful windowed ECDLP ladder (issue #5). Exact (distribution convolution, no sampling). Validates the `2/n` equidistribution heuristic behind the completeness argument — dx=0 collisions track `2/n` within a small constant, even under large accumulator non-uniformity — and surfaces that the *dominant* exceptional term at `w=16` is the zero-window `∞` case (`~2^-11`), not dx=0: a lookup-encoding condition §4 must state (ADR 0008). Cross-checked against a real prime-order curve. |
| `verify/direct_lookup_init.py` | **Circuit-level** demonstration that the ladder's amplitude-1 `∞`-accumulator start is removed structurally (issue #5 part (a), ADR 0009). Reuses the validated controlled-lookup QROM to write `acc ^= T[w]` (`T[w] = [w]·P`) into a `\|0⟩` accumulator and shows the register holds a real affine point for every window, is the `(0,0)` `∞` sentinel **iff `w=0`**, keeps ancilla clean / phase `+1`, and stays `∞` under the `ctrl=0` negative control — so the adder is never fed `∞` at t=0. Exhaustive on a toy prime-order curve (both uncompute modes) + a secp256k1 256-bit spot-check. |
| `verify/offset_window_encoding.py` | **Removes** the dominant exceptional term of the ladder — the zero-window `∞` addend (issue #5 part (b), ADR 0015). Implements the **offset window encoding** (each digit `g → g+1`, one classical correction), proves exhaustively on a real toy curve that it never emits the `∞` table entry yet computes `[a]P+[b]Q` for every `(a,b)`, and re-runs #15's exact measurement to show the `addend=∞` rate is now **exactly 0** while `dx=0` is unchanged — sharpening the completeness headline from `~2⁻¹¹` back to the `dx=0`-limited `~2⁻²⁵⁰`. |
| `verify/mid_ladder_bound.py` | **Exact** end-to-end bound on the ladder's exceptional amplitude (issue #28, ADR 0016). Computes `P[≥1 exceptional across the real 28-window ladder]` by tracking the accumulator's *clean* (never-yet-exceptional) mass and removing exceptional `(acc, window)` pairs at each addition — the exact `P[⋃ A_k]`, shown `≤` the completeness argument's union bound on every config, for both the standard and the `∞`-free offset encoding. Extrapolates to `≈2⁻¹¹` (std) / `≈2⁻²⁵⁰` (offset), both `≪` Shor's `~1%`. |
| `verify/shor_ecdlp_recovery.py` | **Demonstrated attack**: runs the *full* two-register Shor-ECDLP on toy prime-order curves by exact statevector simulation and **actually recovers the secret discrete log `m`** (issue #46, ADR 0019) — using the **incomplete affine adder this circuit implements** (chord-only, `inv(0):=0` misfire) plus the repo's completeness handling (direct-lookup init + offset-window encoding). Three ways: the **complete** adder gives `P_success = (n−1)/n` exactly (harness check); the **offset + incomplete** adder still recovers `m` (only rare `dx=0` misfires, `P_success → (n−1)/n` as exceptions thin with `n`); the **standard + incomplete** adder is wrecked by the zero-window `∞` sentinel fed to the chord formula — showing the offset encoding matters for the *attack*, not only the amplitude bound. Corrupted-basis-state fraction cross-checked against #5's measured exceptional rate. The executable end-to-end complement to the amplitude bound (ADR 0016) and reversible detector (ADR 0018). |
| `verify/toy_shor_qft.py` | **Gate-level capstone**: the *fully gate-level* toy Shor-ECDLP run that **recovers the secret `m`** (issue #55, ADR 0022), unifying the gate-level pieces. A real **gate-level QFT** (explicit H + controlled-phase + bit-reversal-swap gates on a `2^(2w)` statevector — upgrading ADR 0019's analytic DFT) over two `w`-qubit index registers, with the **complete affine point-add** (ADR 0021) as the `[x]P+[y]Q` oracle **applied as a permutation** (the reversible arithmetic is a classical permutation on basis states, so its hundreds of ancilla enter/leave `\|0⟩` *exactly* — omitted, not approximated; a full monolithic Hilbert space over the ancilla is `2^(hundreds)` and impossible). Exact distribution `P(c,d)` (no sampling), rounding recovery `m = round(d·n/2^w)·round(c·n/2^w)⁻¹ mod n` sharpened by `2^w > n²`; recovers `m` on order-7/11 curves, norm `1.0`. The `gate_level_ladder_matches_group_law` `#[cfg(test)]` test in `../src/point_add/toy_shor.rs` (a child module of `toy_pointadd`) grounds the oracle: it chains the ADR 0021 point-add as a gate-level ladder on the sim and asserts `[a]P+[b]Q` equals the reference group law (exceptional pairs included). |
| `cost_model.py` | Maps the real `score.json` + `depth.json` metrics to surface-code physical resources (incl. measured runtime + spacetime volume) under explicit, editable assumptions. |
| `ecdlp_estimate.py` | Derives the **full Shor-ECDLP** cost by composing the measured per-addition primitive with the double-and-add ladder structure (`2(n+1)` additions, windowed variants); replaces the old hand-picked multiplier. Also **consumes** `ladder_measured.json` when present — printing the streamed **emitted+measured** full-ladder totals alongside the derived headline and cross-asserting they agree on the static op-stream basis (issue #27 item 3, ADR 0017). Analysis-only, no `score.json` impact. |
| `ladder_measured.json` | Measured full-ladder artifact (Toffoli reversible/MBUC, toffoli-depth, peak) emitted by `src/point_add/ladder_full.rs`'s streamed w=16 count and consumed by `ecdlp_estimate.py` (issue #27 item 3). Deterministic; regenerate with `LADDER_MEASURED_JSON=analysis/ladder_measured.json cargo test --release --lib full_ladder_streamed_toffoli_qubits_depth -- --ignored`. |
| `../src/bin/depth_report.rs` | Standalone binary: measures toffoli-depth / gate-depth of `ops.bin` via `circuit::analyze_depth`, writes `depth.json`. Does **not** run the simulator or touch `score.json`. |
| `scientific-value.md` | Synthesis: what is proven, the cost mapping, and the generalizable vs. curve-specific techniques. |
| `completeness_argument.md` | Quantitative negligibility argument (issue #5) that the incomplete affine adder suffices for a working Shor run: exceptional-input amplitude `≈ 2⁻²⁵⁰`, >240 bits below Shor's tolerance. |
| `adr/` | Architecture decision records for the analysis layer (isolation from scoring, derived ECDLP estimate). |

## Run everything

Recipes live in the repo-root [`justfile`](../justfile) (`just` — the command
runner — replaces the old `analysis/run.sh`):

```bash
uv sync --locked      # build the locked analysis env (z3, Python 3.11 floor)
uv run just analysis  # z3 proofs + cost model, 14 stages, on the locked venv
uv run just depth     # measure depth -> depth.json (needs ops.bin)
uv run just kani      # Kani proofs on real Rust types (needs cargo kani)
just                  # list every recipe
```

The Python env is **managed by uv** ([`../pyproject.toml`](../pyproject.toml) +
[`../uv.lock`](../uv.lock), transitively hash-pinned — the same reproducibility the
Rust build and ADR trail give the rest of the repo). `uv sync --locked` builds
`.venv` from the lock on the **Python 3.11 floor** (pinned by `../.python-version`),
and `uv run …` puts that venv on `PATH` so each recipe's `python3` uses the locked
z3. Individual stages are recipes too (`just solinas`, `just completeness`,
`just mid-ladder`, `just recover`, …). Without uv, a bare-pip fallback mirrors the
lock: `pip install -r analysis/requirements.txt` (Python 3.11+). `just pycheck`
byte-compiles every analysis script to catch version-incompatible syntax; reproduce
CI's floor check exactly with `uv run just pycheck` (or `just PYTHON=python3.11
pycheck`). The Kani harnesses live behind `#[cfg(kani)]` in
`src/kani_proofs.rs`, so the normal build and `benchmark.sh` never compile them —
zero effect on the score. Every number is produced by a deterministic run; none
are hand-asserted.

## Two-layer verification (why both z3 and Kani)

- **z3** (`verify/*.py`) proves the width-256 arithmetic over abstract
  bitvectors — full field coverage, fast, but a *model* of the algorithm.
- **Kani** (`src/kani_proofs.rs`) proves the exact Rust control flow of the
  Solinas reduction using the real `alloy_primitives::U256` type
  (`solinas_add_u256`, verified against the real secp256k1 prime) and a fast
  small-width twin (`solinas_add_u64`). This binds the proof to the *implementation
  types*, not just the math.
- The division-based `sub_mod` is **not** BMC-tractable (ruint's 256-bit `%` is
  Knuth long division with unbounded loops) — which is itself the argument for
  the division-free Solinas design. That path is covered by the z3 layer.
