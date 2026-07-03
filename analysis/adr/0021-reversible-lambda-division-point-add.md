# ADR 0021 — Reversible λ-division affine point-add with exceptional handling (Path B, toy scale, issue #48)

**Status:** Proposed — *scoping only, not yet implemented.* Records the design and
rationale for the deferred Path B increment (handle, not just detect, the exceptional
cases), so it is captured without committing to the build. Depends on
[ADR 0020](0020-reversible-toy-modular-inverse.md). Promote to **Accepted** when
implemented.
**Date:** 2026-07-03

## Context

This repo's completeness approach is **Path A** (negligibility,
[ADR 0006](0006-adder-completeness-approach.md)): the affine adder's exceptional inputs
are shown **rare and well-characterised** rather than eliminated. Everything built so far
*measures / bounds / detects* them —
[ADR 0008](0008-empirical-completeness-collision-rate.md) (rate),
[ADR 0016](0016-exact-mid-ladder-bound.md) (exact bound),
[ADR 0018](0018-circuit-level-exceptional-detection.md) (reversible detector on real
coordinates) — and [ADR 0019](0019-end-to-end-ecdlp-recovery.md) demonstrates that the
attack **recovers the discrete log** despite them. ADR 0018 and 0019 both explicitly
name the one verb not yet done: *handling* the exceptional cases with a complete /
λ-division affine adder (Path B). This ADR scopes that.

The reason it is deferred, not done: Path B in the **scored** circuit would change
`ops.bin` / the score and contradict ADR 0006. And a faithful λ-division adder needs a
reversible modular inverse, which the analysis layer does not yet have (ADR 0020).

## Decision (proposed)

Build a **reversible affine point-add that *handles* the exceptional branches**, at
**toy width**, as a `#[cfg(test)]` harness in the sim (analysis layer, isolated from the
scored secp256k1 circuit):

1. **Compose ADR 0020's inverse.** `λ = (y₂ − y₁) · inv(x₂ − x₁)` for the generic case;
   `x₃ = λ² − x₁ − x₂`, `y₃ = λ(x₁ − x₃) − y₁`, all as reversible modular arithmetic over
   the toy `F_p`.
2. **Handle, don't misfire, the exceptional branches** — the difference from ADR 0019's
   *modelled* misfire:
   - **doubling** (`P = Q`, `dx = 0`): use the tangent slope `λ = (3x₁² + a)·inv(2y₁)`;
   - **`P = −Q`** (`x₁ = x₂`, `y₁ = −y₂`): output the `∞` sentinel;
   - **`∞` operands**: identity passthrough.
   The branch selection reuses ADR 0018's reversible detector flags (`dx=0`, `∞`-sentinel
   tests) as the controls, so detection and handling share one mechanism.
3. **Reversibility + exhaustive verification.** Over **every** `(P, Q)` pair of a real
   prime-order toy curve (ADR 0018's orders 19/29/41): output equals the reference group
   law on all pairs *including the exceptional ones*, all ancilla clean, phase `+1`.
   Contrast with ADR 0019, where those pairs are the ones that misfire — here they are
   handled correctly.
4. *(Further stretch — separate issue if pursued.)* Drop this complete adder into the
   existing `qaddend_testbed` / `ladder_stream` harness and add a gate-level QFT for a
   **fully gate-level toy Shor** run — the union of all currently-separate gate-level
   pieces (arithmetic proofs, QROM lookup, quantum-addend add, detector, and now a
   complete point-add). Explicitly out of scope for this ADR.

## Consequences (anticipated)

- **Closes the "detect vs handle" gap at the circuit level.** Path A's detector
  (ADR 0018) becomes a *complete adder*; the exceptional inputs are eliminated in the
  toy circuit, not just bounded — an optional strengthening beyond what any current claim
  needs.
- **Does not alter the project's Path-A choice for the scored circuit.** Toy-width,
  analysis-layer only; the secp256k1 primitive and the estimate are untouched
  (`ops.bin` byte-identical, [ADR 0001](0001-analysis-layer-isolated-from-score.md)).
  Path B here is a *demonstration that it can be done*, not a switch of approach
  (ADR 0006 stands).
- **Marginal value is modest, effort is high.** Arithmetic is machine-checked (z3+Kani),
  the exceptional set is detected (ADR 0018), and recovery is demonstrated (ADR 0019);
  this adds gate-level *handling*, valuable but not load-bearing. Gated behind ADR 0020.
  Priority: after the paper writeup + uv migration. Remains Proposed until built.
