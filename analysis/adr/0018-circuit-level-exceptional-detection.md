# ADR 0018 — Circuit-level EC exceptional detection over real coordinates (completeness, issue #28/#5)

**Status:** Accepted — decides how to deliver #28's remaining item (the
*circuit-level* mid-ladder exceptional confirmation over real coordinate
arithmetic); implemented in `src/point_add/ec_exceptional.rs`.
**Date:** 2026-07-03

## Context

The completeness axis of #5 is substantially closed, but entirely in the
**scalar/dlog model**:

- The negligibility argument (#14, `completeness_argument.md`) and the exact
  end-to-end bound ([ADR 0016](0016-exact-mid-ladder-bound.md),
  `mid_ladder_bound.py`) both represent a point by its discrete log `s ∈ Z_n`,
  with `∞ = 0` and the affine collision `dx=0` modelled as `acc ≡ ±addend (mod n)`.
- The offset encoding ([ADR 0015](0015-offset-window-encoding.md)) pins the
  zero-window `∞` term, again in the scalar model.

That whole edifice rests on one *curve* fact — **two points share an x-coordinate
iff they are negatives** (`t ≡ ±s`). #15 cross-checked it against a real curve in
Python. What #28 still asks for is a **circuit-level** confirmation: a reversible
detector operating on real `(x, y)` coordinate qubits, over a real prime-order
curve, agreeing with the scalar predicate on the actual superposition — moving the
bound from the scalar/dlog abstraction to real coordinate arithmetic, on the
quantum-addend substrate the rest of Tier B now uses (ADR 0014, `ladder_stream.rs`).

## Decision

Build the confirmation around the key simplification that the exceptional set is
**detectable without the full point-add**: `dx = 0` is exactly `x1 == x2`
(bitwise x-coordinate equality), and the `∞` cases are sentinel zero-tests. No
modular inverse or λ-division is needed to *detect* (or to *bound*) the exceptional
inputs — only to *compute* the addition, which does not change *which* inputs are
exceptional. So a small reversible circuit suffices. Concretely,
`src/point_add/ec_exceptional.rs` (a `#[cfg(test)]` harness):

1. **Real toy curve.** `y² = x³ + ax + b` over `F_p`; enumerate points, require the
   group order `n` to be **prime** (asserted — a loud failure if the params don't
   give a prime-order curve), pick a generator `G`, and tabulate `[k]G` for all `k`
   (so `⟨G⟩ ≅ Z_n` and every point has a known dlog). `∞` is the off-curve `(0,0)`
   sentinel.
2. **Reversible detector on the `B` emitter.** `dx0 = (x1==x2)` via an XOR-equality
   + zero-test; `acc_inf = ((x1,y1)==(0,0))` and `add_inf = ((x2,y2)==(0,0))` via
   `∞`-sentinel zero-tests. All scratch returns to |0>; the three flags carry the
   basis-diagonal verdicts. Simulation-verified on crafted inputs (generic, `P==Q`,
   `P==−Q`, `acc=∞`, `addend=∞`).
3. **Model confirmation.** Measure the detector over **all** `(accumulator, addend)`
   coordinate pairs of the curve (masked multi-shot) and assert the real-coordinate
   verdict equals the scalar/dlog predicate `(m==0) ∨ (y==0) ∨ (y ≡ ±m)` on **every**
   pair — the circuit-level confirmation of the model the whole bound rests on.
4. **End-to-end residual on the real ladder.** Drive the ADR 0016 survival recursion
   with the **circuit-measured** predicate over the real two-scalar `[a]P+[b]Q` toy
   ladder; report the exact mid-ladder residual, confirm `exact ≤ union`, and confirm
   the **offset** encoding yields `add_inf = 0` at every window on real coordinates
   (the zero-window-`∞` pin, ADR 0015, circuit-confirmed).

**Why a reduced toy curve (not secp256k1).** The exceptional *predicate* is a
property of coordinates, not of the width — `x1==x2` and the `∞` sentinels are
width-parametric. A prime-order toy curve lets the check be **exhaustive** over the
entire group (every `(y, m)` pair), which a 256-bit curve cannot. This mirrors
ADR 0014's reduced-width justification and ADR 0016's toy configs.

**Why detection, not the full λ-division point-add.** Completeness is about the
*rate of exceptional inputs*, which the detector measures exactly; building the
reversible modular-inverse point-add would add a large circuit that does not change
the exceptional set. That full point-add is a separate, larger increment (called
out, not silently skipped).

## Consequences

- **#28's remaining item is delivered at the circuit level.** The scalar/dlog exact
  bound (ADR 0016) and the offset-encoding pin (ADR 0015) are now confirmed by a
  reversible circuit over **real coordinate arithmetic**, on a real prime-order
  curve, exhaustively over the whole group — not only in the dlog abstraction or by
  a Python classical cross-check.
- **The exceptional set is shown reversibly detectable.** `dx=0 / acc=∞ / addend=∞`
  are flagged by a small ancilla-clean circuit — the same signal a real ladder would
  use to detect (and a completeness proof to bound) the bad inputs.
- **Scope (honest).** Still a reduced toy curve, and the detector *detects* the
  exceptional inputs rather than *handling* them with complete formulas; the
  reversible λ-division point-add (and its exceptional-branch handling) is a separate
  increment. The scalar model is now confirmed on real coordinates, which is what the
  completeness bound needs.
- Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md): the harness
  is `#[cfg(test)]`, never compiled into `build_circuit`; the scored circuit is
  byte-identical (`ops.bin` SHA unchanged).
