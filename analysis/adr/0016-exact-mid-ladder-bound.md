# ADR 0016 — Exact end-to-end bound on the mid-ladder exceptional amplitude

**Status:** Accepted — replaces the per-addition union bound with an exact
end-to-end amplitude; confirms it never exceeds the union bound and stays ≪ Shor's
tolerance under both encodings
**Date:** 2026-07-03

## Context

The completeness argument ([issue #5](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/5),
`completeness_argument.md §4`) bounds the incomplete affine adder's exceptional
amplitude across the 28-window ladder by a **union bound** —
`P[exceptional] ≤ Σ_k P[exceptional at addition k] ≈ 28·2/n`. `#15` / ADR 0008
measured those per-addition rates exactly; `#34` / ADR 0015 removed the dominant
zero-window `∞` term with an offset encoding. [Issue #28](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/28)
asks for the *exact* amplitude on the real two-scalar superposition — the
probability that **any** addition is exceptional — "rather than a per-addition +
union bound".

## Decision

Add `analysis/verify/mid_ladder_bound.py` (suite stage 9/11): compute the exact
`P[⋃_k A_k]` by tracking the accumulator distribution restricted to the **clean**
mass (no exceptional at any prior addition). At each windowed addition the
`(accumulator, window-value)` pairs that are exceptional (`addend = ∞` when
`v·c ≡ 0`; `acc = ∞` when `acc ≡ 0`; `dx = 0` when `acc ∈ {M, −M}`) are removed
into a running failed total, and the surviving clean mass convolves forward. The
union bound (`Σ_k P[A_k]`) is computed in parallel from the *unrestricted*
distribution (this repo's #15 quantity) for comparison. Exact rationals
(`Fraction`); the scalar model (point ↔ discrete log; `dx=0 ⇔ acc ≡ ±M`) is the
one validated against a real curve in #15. Reported for both the **standard** and
the **offset** (`ADR 0015`) encodings.

## Consequences

- **Exact ≤ union, always** (asserted on every config) — the completeness
  argument's union-bounded headline is a valid upper bound, now with the exact
  end-to-end amplitude underneath it.
- **Mass conservation** (`exact + survival == 1`, exactly) confirms the tracking
  loses no probability — the "exact" figure is exact, not an approximation.
- **The offset encoding's benefit is end-to-end, not just per-addition**: offset
  exact `<` standard exact on every config (the zero-window `∞` term is gone).
- **At attack parameters** (`n≈2²⁵⁶`, `w=16`, 28 additions) an exact convolution
  is infeasible (`2²⁵⁶` distribution entries), so the rigorous end-to-end bound is
  the analytic **union upper bound** (`P[⋃ A_k] ≤ Σ P[A_k]`), evaluated in closed
  form: `≈ 2⁻¹¹` (standard, ∞-dominated) or `2⁻²⁵⁰` (offset, `dx=0`-limited) — both
  ≪ Shor's `~1%` tolerance. The toy `exact ≤ union` results certify this bound is
  rigorous and not loose. (Toy-scale rates are large by design: they scale as
  `2/n` and `1/2^w`, tiny only at attack scale.)
- This closes the "union bound → exact bound" half of issue #28's definition of
  done, and pairs with #34's pinned zero-window encoding. The remaining #28 item —
  a *circuit-level* mid-ladder demonstration over real coordinate arithmetic —
  rides on the quantum-addend testbed ([issue #27](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/27),
  ADR 0014), whose modular-add tail is now in place.
- Analysis-only, deterministic, pure-Python; consistent with
  [ADR 0001](0001-analysis-layer-isolated-from-score.md) — no effect on the scored
  circuit.
