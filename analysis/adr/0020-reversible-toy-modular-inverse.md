# ADR 0020 — Reversible toy-width modular inverse (Path B prerequisite, issue #48)

**Status:** Proposed — *scoping only, not yet implemented.* Records the design and
rationale for the reversible modular inverse that Path B (ADR 0021) depends on, so
the increment is captured without committing to the build. Promote to **Accepted**
when implemented.
**Date:** 2026-07-03

## Context

The completeness axis is closed via **Path A** (negligibility, [ADR 0006](0006-adder-completeness-approach.md)):
the exceptional inputs are bounded exactly ([ADR 0016](0016-exact-mid-ladder-bound.md)),
detected reversibly on real coordinates ([ADR 0018](0018-circuit-level-exceptional-detection.md)),
and the full attack recovers the discrete log end-to-end at toy scale
([ADR 0019](0019-end-to-end-ecdlp-recovery.md), #46). All of those *detect or bound*
the exceptional cases. **Handling** them with a complete/λ-division affine adder —
Path B — is the deferred increment, and it needs one primitive the analysis layer does
not yet have: a **reversible modular inverse** over `F_p`.

The scored circuit's real inverse is a 256-bit Bernstein–Yang / Kaliski conjugate
uncomputation (`mod.rs`, `gcd.rs`) that the z3 layer covers but Kani cannot BMC
(unbounded division-dependent loops — the negative result in `scientific-value.md §1c`).
It is not portable to toy width as-is, and building the affine λ-division adder (ADR 0021)
without an inverse circuit is impossible. This ADR scopes that inverse as its own,
exhaustively-verifiable, toy-width artifact.

## Decision (proposed)

Build a **reversible toy-width modular inverse** as a `#[cfg(test)]` harness in the sim
(analysis layer, never compiled into `build_circuit`):

1. **Small prime field `F_p`** (the ADR 0018/0019 toy curves' base field, `p ~ 17–43`),
   registers a handful of bits wide, so verification is **exhaustive** over the whole
   field — the same reduced-width justification as ADR 0014/0016/0018.
2. **Kaliski / EEA-style inverse** `inv(a) = a^{-1} mod p` for `a ≠ 0`, built from the
   repo's existing reversible primitives (Cuccaro add/sub, comparator, controlled swap)
   plus a bounded almost-inverse loop with a fixed toy iteration count (the width is
   tiny, so the loop bound is a small constant — exactly why toy width is BMC/exhaustive
   -tractable where 256-bit is not).
3. **`inv(0)` well-defined** (the exceptional value): the circuit must produce a
   deterministic, documented result for `a = 0` (the `inv(0):=0` convention ADR 0019
   models classically, or an explicit flag), so the λ-division adder (ADR 0021) can
   branch on it to *handle* the `dx=0` exception rather than misfire.
4. **Verification.** Exhaustive simulation over all `a ∈ F_p`: `inv(a)·a ≡ 1 (mod p)`
   for `a ≠ 0`; the defined `inv(0)` behaviour; all scratch/ancilla returned to `|0⟩`
   (reversibility, `emit_inverse`-safe); global phase `+1`. Optionally an SMT/z3 lemma
   on the toy-width relation to mirror the arithmetic-proof layer.

**Why a separate ADR/artifact from the point-add.** The inverse is the crux and the
single largest sub-circuit; isolating it lets it be verified exhaustively on its own
before the λ-division adder composes it. It is also independently useful (any modular
division at toy width).

## Consequences (anticipated)

- **Unblocks Path B (ADR 0021).** The λ-division affine adder becomes buildable.
- **Toy width only, by design.** This is *not* a port of the 256-bit inverse and does
  not touch the scored circuit or the estimate's rigor; it is the reduced-width
  substrate for exhaustive Path-B verification, consistent with ADR 0018's reasoning.
- **Complements, does not replace, the arithmetic proofs.** z3 + Kani already prove the
  scored Solinas add over all inputs; this adds a *different* operation (inverse) at a
  *verifiable* width, not a re-proof of existing arithmetic.
- **Isolation preserved** ([ADR 0001](0001-analysis-layer-isolated-from-score.md)):
  `#[cfg(test)]`, `ops.bin` byte-identical.
- **Effort/priority.** High-effort; scheduled after the paper writeup + uv migration.
  Remains Proposed until built — a design record, not a commitment.
