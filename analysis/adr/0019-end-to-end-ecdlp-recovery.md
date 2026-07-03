# ADR 0019 — End-to-end Shor-ECDLP discrete-log recovery on toy curves (demonstrated attack, issue #46)

**Status:** Accepted — decides how to demonstrate a *full* ECDLP recovery (not
only bound/detect the exceptional set); implemented in
`analysis/verify/shor_ecdlp_recovery.py`.
**Date:** 2026-07-03

## Context

Every completeness result so far stops short of the payload. The negligibility
argument (#5, `completeness_argument.md`) and the exact end-to-end bound
([ADR 0016](0016-exact-mid-ladder-bound.md)) *bound* the exceptional amplitude;
the reversible detector ([ADR 0018](0018-circuit-level-exceptional-detection.md))
*detects* the exceptional set on real coordinates. None of them shows the thing an
attacker actually wants: that the full Shor-ECDLP pipeline, **using the incomplete
affine adder this circuit implements** plus the repo's exceptional-case handling,
**recovers the secret discrete log**. That is the repo's sharpest stated
non-claim (`novelty-assessment.md`: *"No demonstrated end-to-end attack"*).

This ADR closes that gap at toy scale, by exact statevector simulation, so the
completeness story is validated by *recovering the secret*, not only by an
amplitude bound.

## Decision

`analysis/verify/shor_ecdlp_recovery.py` (analysis-only, pure-Python, deterministic;
reuses `Curve` / `find_prime_order_curve` / the offset machinery already validated
in #5/#15) runs the standard two-register Shor-ECDLP on small prime-order toy curves
and computes the **exact** measurement distribution — no Monte-Carlo sampling.

1. **State.** Two index registers over `Z_n` in uniform superposition; an oracle
   writes the point register `|a⟩|b⟩|O⟩ → |a⟩|b⟩|R(a,b)⟩` with `R(a,b) = [a]P +
   [b]Q`. QFT_n on both registers, then the exact Born-rule distribution
   `P(c,d) = Σ_R |(1/n²) Σ_{(a,b):R(a,b)=R} ω^{ca+db}|²`, ω = e^{2πi/n}.
2. **Classical recovery.** For a measured `(c,d)` with `c ≠ 0` (always invertible
   mod the prime `n`), `m = d·c⁻¹ (mod n)`; success ⇔ `d ≡ c·m`. For the ideal
   oracle `P_success = (n−1)/n` exactly — used to validate the harness.
3. **The oracle is the incomplete adder, three ways.** `R(a,b)` is built by the
   windowed affine ladder (direct-lookup init writes `acc`; each later window adds a
   precomputed multiple), and the per-addition `adder` is swapped to isolate the
   completeness handling:
   - **complete** — reference group law (`Curve.add`): the ideal, `P_success =
     (n−1)/n`.
   - **offset + incomplete** — chord-only adder with the circuit's `inv(0):=0`
     misfire convention, over the **offset digit set** (`g→g+1`, ADR 0015) so the
     addend is never the `∞` sentinel and direct-lookup keeps `acc` finite: the only
     residual exceptions are `dx=0` collisions.
   - **standard + incomplete** — same adder over the standard digit set, where a
     zero window selects the `[0]·P = ∞` sentinel `(0,0)` and feeds it to the chord
     formula, corrupting the result.
4. **Assertions (locked).** complete `P_success == (n−1)/n` (exact); offset recovers
   the true `m` (distribution mode on the correct `d ≡ cm` line) with `P_success`
   far above chance; standard is **strictly worse** than offset (the zero-window `∞`
   term damages recovery, not just the amplitude bound). The count of corrupted
   `(a,b)` basis states is cross-checked against the exact exceptional rate of
   `completeness_collision_rate.py`.

### The `inv(0):=0` misfire model

The scored point-add uses a chord/tangent affine addition; on `dx=0` its
modular-inverse step has no inverse. Modelling the misfire as slope `λ = (y₂−y₁)·
inv(dx)` with `inv(0):=0` makes the incomplete adder **exactly** the group law when
`dx≠0` and a deterministic wrong point when `dx=0` — a faithful, deterministic stand-in
for "the affine adder corrupts the output on the exceptional state" (the gating
experiment's finding, #5 §3). The `∞` addend is represented by the same `(0,0)`
sentinel the circuit uses, and is **not** special-cased in the incomplete adder, so
the standard encoding reproduces the zero-window corruption the offset encoding removes.

## Consequences

- **The "no demonstrated attack" non-claim is retired at toy scale.** The pipeline
  recovers the secret `m` from `Q=[m]P`, using the incomplete affine adder + offset
  encoding + direct-lookup init — an executable end-to-end confirmation that the
  completeness handling *works*, complementing the amplitude bound (ADR 0016) and the
  reversible detector (ADR 0018).
- **The offset encoding's value is shown for the attack, not only the bound.** Standard
  vs offset recovery probability separates cleanly: the zero-window `∞` term, which
  ADR 0015 removed structurally, is the term that most degrades an actual run.
- **Scope (honest), and why it is the right scope.**
  - *Toy scale, exact statevector.* `n ~ 19–100`; the exact `P(c,d)` needs `O(n⁴)`
    work (or `O(n³)` for `P_success` alone), so the demonstration is small by
    construction. The 256-bit attack stays out of reach — that is what the *estimate*
    (`ecdlp_estimate.py`) is for; this is the qualitative "does it invert the ECDLP"
    complement, with a closed-form scale-up of the exceptional rate already given by
    #5/#28.
  - *Group-law level, not a gate-level toy circuit.* The oracle computes points via
    the (incomplete) group law, exactly modelling the affine adder's exceptional
    behaviour; the *gate-level* rigor lives elsewhere (the secp256k1 scored primitive,
    the z3/Kani arithmetic proofs, and the ADR 0018 reversible real-coordinate
    detector). Building a general toy-curve reversible Shor circuit with a QFT is a
    separate, larger increment, called out rather than silently skipped.
- Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md): analysis
  layer only, `#[cfg(test)]`-equivalent (a `verify/` script), never compiled into
  `build_circuit`; the scored circuit is byte-identical (`ops.bin` SHA unchanged).
