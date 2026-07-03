# ADR 0022 — Gate-level QFT toy Shor-ECDLP capstone: unifying the gate-level pieces (issue #55)

**Status:** Accepted — implemented in `analysis/verify/toy_shor_qft.py` (gate-level QFT
recovery) plus the `gate_level_ladder_matches_group_law` `#[cfg(test)]` test in
`src/point_add/toy_pointadd.rs` (the complete point-add chained as a gate-level ladder
oracle). The "further stretch" called out in [ADR 0021](0021-reversible-lambda-division-point-add.md) §4.
Depends on [ADR 0019](0019-end-to-end-ecdlp-recovery.md), [ADR 0021](0021-reversible-lambda-division-point-add.md).
**Date:** 2026-07-03

## Context

The gate-level rigor in this repo has, until now, lived in **separate** pieces:
the z3/Kani arithmetic proofs; the QROM windowed lookup ([ADR 0010](0010-measured-windowed-lookup-cost.md));
the quantum-addend point-add testbed ([ADR 0014](0014-quantum-addend-testbed.md));
the reversible real-coordinate exceptional detector ([ADR 0018](0018-circuit-level-exceptional-detection.md));
the reversible modular inverse ([ADR 0020](0020-reversible-toy-modular-inverse.md)); and the
complete λ-division affine point-add ([ADR 0021](0021-reversible-lambda-division-point-add.md)).
The end-to-end recovery ([ADR 0019](0019-end-to-end-ecdlp-recovery.md)) demonstrated the
*attack* — recovering the secret `m` — but at the **group-law level**: its oracle used the
reference/incomplete group law and its "QFT" was the analytic DFT, not a gate-level circuit.
ADR 0019 §Scope and ADR 0021 §4 both name the one increment not yet done and call it out
rather than skip it: **a fully gate-level toy Shor run that unifies all the gate-level
pieces** — the arithmetic oracle *and* the QFT built from explicit gates.

This ADR closes that gap and is the capstone of the analysis layer.

## The feasibility question (and its exact answer)

A naïve reading of "fully gate-level" is: build one statevector over **every** qubit —
index registers *and* the hundreds of reversible-arithmetic ancilla — and run every gate
on it. That is impossible: the Hilbert space is `2^(hundreds)`. And the ancilla cannot
simply be dropped, because mid-oracle they are **entangled** with the index superposition
(the arithmetic is only reversible/clean *after* the compute→copy→uncompute completes).

The exact, standard resolution — not an approximation:

> A reversible arithmetic oracle is a **classical permutation** on basis states. On each
> index basis state `|x,y⟩` it maps `|x,y,0…0⟩ → |x,y, R(x,y), 0…0⟩` — the ancilla enter
> and leave `|0⟩` **exactly** (they uncompute). So the oracle can be applied per index
> basis state, and the statevector need only span the small **index registers**. The
> ancilla are omitted *exactly*, not traced-out approximately.

On those small index registers a **real gate-level QFT** (H + controlled-phase + bit-reversal
swaps) then runs on a genuine statevector. This is tractable (`2^(2w)` amplitudes, `w≈7–8`)
and is a faithful gate-level QFT — the piece ADR 0019 did analytically.

## Decision

Two artifacts, unified:

1. **`analysis/verify/toy_shor_qft.py`** — the gate-level QFT recovery, pure-Python,
   deterministic, exact (no sampling):
   - two `w`-qubit index registers `x, y` in uniform superposition over `Z_{2^w}`;
   - the oracle `[x]P + [y]Q` applied **as a permutation** (ancilla exactly omitted):
     the point register collapses to `k = (x + m·y) mod n`, so for each `k` the
     basis-aligned component is `1/2^w` on the `(x,y)` with `(x + m·y) mod n == k`;
   - a **gate-level QFT** on each index register — `apply_h` + `apply_cphase` +
     `apply_swap`, the textbook circuit (H, then controlled-`R_{k}`, then bit-reversal),
     applied to a flat `2^(2w)` statevector;
   - the exact distribution `P(c,d)` accumulated over `k` by tracing out the measured
     point register (`|amplitude|²` summed), then classical **rounding recovery**
     `j = round(c·n/2^w)`, `e = round(d·n/2^w)`, `m = e·j⁻¹ (mod n)` — valid because
     `2^w > n²` sharpens the phase-estimation peaks (the standard Shor guarantee).
   - **Locked result:** recovers the true `m` from the peak outcome on prime-order toy
     curves (order 7, `w=7`, `m=3`; order 11, `w=8`, `m=4`); distribution norm `1.0`;
     `P(correct m)` ≈ 0.84 / 0.89 over all informative outcomes.

2. **`gate_level_ladder_matches_group_law`** (Rust `#[cfg(test)]`, `toy_pointadd.rs`) —
   grounds the "arithmetic oracle" the Python relies on. It builds `[a]P + [b]Q` by
   **chaining the ADR 0021 complete point-add** as a gate-level ladder on the bit-sliced
   sim (`emit_point_add` per step, accumulator threaded register-to-register, constants
   loaded via `load_const`), then asserts the sim readback equals the reference group law
   `ec_add(ec_mul(a,gen), ec_mul(b,qbase))` — over an order-7 curve with secret `m=3`,
   including the exceptional pairs (∞ start, doublings, `→∞`). This is the gate-level
   evidence that the permutation `toy_shor_qft.py` applies is exactly the group-law oracle.

## Why this is the honest form of "fully gate-level"

- The **QFT is genuinely gate-level** (H + controlled-phase + swaps on a statevector),
  upgrading ADR 0019's analytic DFT.
- The **oracle is genuinely gate-level** — the ADR 0021 complete point-add, chained and
  verified against the group law in the Rust test — but is *applied as a permutation* on
  the index basis states, which is exact (ancilla uncompute to `|0⟩`), not an approximation.
- The one thing that is **not** done, stated plainly: a single monolithic statevector over
  index registers *and* arithmetic ancilla with every gate run in one Hilbert space. That
  is `2^(hundreds)` and impossible at any scale; the permutation-oracle decomposition is
  the exact technique real Shor simulators use, not a shortcut.

## Consequences

- **The gate-level pieces are unified into one run.** ADR 0021 §4's "further stretch" is
  delivered: a fully gate-level toy Shor that recovers the discrete log, with a real
  gate-level QFT and the complete point-add as its (permutation-applied) oracle.
- **Complements, does not replace, ADR 0019.** ADR 0019 remains the group-law-level study
  of the *incomplete* adder and the offset encoding's value; this ADR is the gate-level-QFT
  recovery with the *complete* adder. Different questions, both answered.
- **Marginal value modest, honestly scoped.** The 256-bit attack stays out of reach (that
  is what `ecdlp_estimate.py` is for); this is the qualitative gate-level capstone. Toy
  scale, exact, deterministic.
- **Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md).** Analysis
  layer only — a `verify/` script and a `#[cfg(test)]` test; never compiled into
  `build_circuit`. The scored secp256k1 circuit is byte-identical (`ops.bin` SHA
  `f30d8365c1235002`, unchanged).
