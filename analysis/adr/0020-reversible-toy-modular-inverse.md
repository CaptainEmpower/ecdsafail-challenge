# ADR 0020 — Reversible toy-width modular inverse (Path B prerequisite, issue #48)

**Status:** Accepted — implemented in `src/point_add/toy_field.rs` (`#[cfg(test)]`).
Verified exhaustively over `F_p` for `p ∈ {5,7,11,13,17,19,23}`.
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
2. **Reversible modular inverse** `inv(a) = a^{-1} mod p` for `a ≠ 0`, built entirely
   from the repo's validated primitives, verified exhaustively.
3. **`inv(0)` well-defined** (the exceptional value): the circuit produces `inv(0)=0`
   (the convention ADR 0019 models classically), so the λ-division adder (ADR 0021) can
   branch on it to *handle* the `dx=0` exception rather than misfire.
4. **Verification.** Exhaustive simulation over all `a ∈ F_p`: `inv(a)·a ≡ 1 (mod p)`
   for `a ≠ 0`; `inv(0)=0`; all scratch/ancilla returned to `|0⟩` (reversibility);
   global phase `+1`.

## As built (implementation reality)

**Fermat, not Kaliski.** The inverse is realized as `a^{p-2} mod p` by left-to-right
square-and-multiply over the fixed classical exponent `p-2`, using a from-scratch
reversible **modular multiply** (`mod_mul`, schoolbook double-and-add) built on the
validated VBE modular adder (`qaddend_testbed::mod_add`, ADR 0014). Kaliski/EEA is the
*space-optimal* choice for the real 256-bit inverse (Roetteler et al.), but at toy width
Fermat-via-multiply is markedly simpler to build correctly and to verify exhaustively,
and it is equally a **reversible-arithmetic** inversion (not a table) — the property this
ADR is about. The space cost is irrelevant here (analysis-layer, toy width); if a future
increment needs a width-faithful inverse it would swap in Kaliski behind the same
interface.

**Uncomputation by op-reversal.** Because the whole gadget is built from `X`/`CX`/`CCX`
only (all involutions), a forward-only fragment is cleanly uncomputed by re-emitting its
op list reversed. Every gadget is *compute → copy result into a clean `out` → reverse*,
so `out ^= f(inputs)` with inputs preserved and all scratch returned to `|0⟩` — no
hand-written inverse circuits. `mod_mul` / `mod_inv` are exposed as composable clean
gadgets (`emit_mod_mul` / `emit_mod_inv`) for ADR 0021.

**Why a separate ADR/artifact from the point-add.** The inverse (and the multiply it
needs) is the crux; isolating it lets it be verified exhaustively on its own before the
λ-division adder composes it. Both are independently useful (any modular division/multiply
at toy width).

## Consequences (anticipated)

- **Unblocks Path B (ADR 0021).** The λ-division affine adder becomes buildable.
- **Toy width only, by design.** This is *not* a port of the 256-bit inverse and does
  not touch the scored circuit or the estimate's rigor; it is the reduced-width
  substrate for exhaustive Path-B verification, consistent with ADR 0018's reasoning.
- **Complements, does not replace, the arithmetic proofs.** z3 + Kani already prove the
  scored Solinas add over all inputs; this adds a *different* operation (inverse) at a
  *verifiable* width, not a re-proof of existing arithmetic.
- **Isolation preserved** ([ADR 0001](0001-analysis-layer-isolated-from-score.md)):
  `#[cfg(test)]`, `ops.bin` byte-identical (SHA `f30d8365…` unchanged).
- **Done.** `mod_mul` and `mod_inv` verified exhaustively over `F_p`
  (`p ∈ {5,7,11,13,17,19,23}`): `out=(x·y) mod p` / `inv(a)=a^{p-2}`, inputs preserved,
  all scratch `|0⟩`, phase `+1`. `emit_mod_mul` / `emit_mod_inv` are ready for ADR 0021.
