# ADR 0021 — Reversible λ-division affine point-add with exceptional handling (Path B, toy scale, issue #48)

**Status:** Accepted — implemented in `src/point_add/toy_pointadd.rs` (`#[cfg(test)]`),
on the ADR 0020 field arithmetic. Verified exhaustively over every `(P,Q)` pair of
prime-order toy curves of order 19/29/41. Depends on
[ADR 0020](0020-reversible-toy-modular-inverse.md).
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

## As built

Implemented as one *compute → copy → reverse* gadget (every op `X`/`CX`/`CCX`, so the
forward fragment re-emitted reversed uncomputes all scratch — the ADR 0020 pattern):

- **Forward:** ∞ flags via the ADR 0018 zero-tests (`P=∞ ⇔ (x1,y1)=(0,0)`, likewise
  `Q`); `dx=x2−x1`, `dy=y2−y1`, `sy=y1+y2`; `eqx`, `dbl=(P==Q)`, `neg=(P==−Q)`; the
  slope numerator/denominator as chord `(dy,dx)` **plus** the tangent `(3x1²+a, 2y1)`
  gated on `dbl` (for a true double `dx=dy=0`, so the gated add yields exactly the
  tangent); `λ = num · inv(den)` (ADR 0020 `mod_inv`+`mod_mul`); `x3=λ²−x1−x2`,
  `y3=λ(x1−x3)−y1`. The generic denominator is never 0; `inv(0)=0` keeps the overridden
  ∞/neg branches from dividing by zero.
- **Copy (mux):** mutually-exclusive controls `c_pinf`/`c_qinf`/`c_gen` select
  `Q` / `P` / `(x3,y3)` into the clean output; `P=−Q` selects nothing → `(0,0)=∞`.
- **Verification:** exhaustive over **every** `(P,Q)` pair of three real prime-order
  toy curves — `y²=x³+2x+2/F₁₇` (order 19, 361 pairs, 73 exceptional),
  `x³+x+4/F₂₃` (order 29, 841 / 113), `x³+x+3/F₃₁` (order 41, 1681 / 161). Every result
  equals the reference group law (chord, tangent, and all ∞/−P branches), inputs
  preserved, all scratch `|0⟩`, phase `+1`.

## Consequences

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
  this adds gate-level *handling*, valuable but not load-bearing. Built on ADR 0020.
- **Done.** The affine adder is **complete** on real toy curves — the Path-B "handle,
  not just detect/bound" increment delivered. The fully gate-level toy Shor run (item 4
  above) remains a further stretch, out of scope here.
