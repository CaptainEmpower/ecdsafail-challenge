# Completeness negligibility argument (issue #5, Path A)

This turns the ECDLP result from a *cost estimate* toward a *verified attack* by
arguing — quantitatively — that the incomplete affine adder this repo implements
is sufficient for a working Shor-ECDLP run, i.e. the exceptional cases it
mishandles occur with amplitude far below Shor's tolerance. This is the
Roetteler–Naehrig–Svore–Lauter 2017 style argument, made concrete with this
repo's measured behaviour ([ADR 0006](adr/0006-adder-completeness-approach.md)).

It is an **argument, not a machine-checked proof**; the caveats at the end state
exactly what is heuristic.

## 1. Shor's tolerance to a small wrong fraction

Shor's period-finding succeeds as long as all but a small fraction of the
computational basis states are computed correctly (in value **and** phase). The
source paper (Babbush et al. 2026, Appendix A.5) proves only ~99% correctness via
Fiat–Shamir fuzz and relies on exactly this: "a superposition with 1% of points
in the wrong place will cause the algorithm to fail at most 1% of the time." So
it suffices to show the total amplitude landing on an exceptional adder input is
≪ 1% (and can be repeated a few times to overcome any residual failure).

## 2. What the adder does on exceptional inputs (measured)

`src/point_add/completeness_probe.rs` ran the built circuit on crafted
exceptional inputs (16 RNG seeds). The signature is uniform:

| input | ancilla | output | phase |
|---|---|---|---|
| doubling (dx=0) | clean 16/16 | wrong | corrupted on ~9/16 |
| P=−Q (dx=0) | clean 16/16 | wrong | corrupted on ~4/16 |
| ∞ accumulator | clean 16/16 | wrong | corrupted on ~8/16 |

Key facts used below: exceptional inputs **do not leak the ancilla** (no
register-basis corruption of *other* basis states — the error is confined to the
offending state), but they **do** inject wrong output and probabilistic phase
garbage. So each exceptional basis state contributes **at most its own
amplitude** to Shor's failure probability. It is therefore enough to bound the
summed amplitude of exceptional inputs.

## 3. The ∞-accumulator must be removed structurally (not by negligibility)

The running accumulator starts at ∞ with **amplitude 1** (before any addition),
so ∞ is *not* rare at the start and cannot be waved away. The paper's ladder
removes it structurally: the **first windowed addition is replaced by a direct
table lookup** (Appendix A) that *writes* the initial accumulator instead of
adding into ∞. Hence the adder is never fed ∞ as the accumulator at t=0.

After the first window, the accumulator equals `[a']P + [b']Q` for the
partial scalars accumulated so far; it is ∞ only when that partial scalar is
`≡ 0 (mod n)`, i.e. **one** value out of the group order `n ≈ 2²⁵⁶` — amplitude
`~1/n ≈ 2⁻²⁵⁶` per addition, which falls under the §4 bound. So the ∞ case is
handled: amplitude-1 start removed by construction, residual occurrences
negligible.

> **This is now circuit-demonstrated, not only assumed** (issue #5 part (a),
> [ADR 0009](adr/0009-direct-lookup-init.md)). `verify/direct_lookup_init.py`
> builds the direct-lookup init as an actual reversible circuit — the validated
> controlled-lookup QROM (§1e / issue #3) writing `acc ^= T[w]` with
> `T[w] = [w]·P` into a `|0⟩` accumulator — and verifies, exhaustively on a toy
> prime-order curve and at secp256k1's 256-bit width, that the accumulator holds
> a real affine point for every window and is the `(0,0)` ∞ sentinel **iff
> `w = 0`** (ancilla clean, phase `+1`; a `ctrl=0` negative control leaves it at
> ∞). So the adder is never fed the amplitude-1 ∞ start; the only residual is the
> `w=0` zero-window term (issue #5 part (b)) — and that term is now removed
> *structurally* by the offset window encoding below, not merely bounded. The
> end-to-end check of the *mid-ladder* residual over the real 28-window two-scalar
> superposition is now **done**: [ADR 0016](adr/0016-exact-mid-ladder-bound.md)
> computes it exactly and [ADR 0018](adr/0018-circuit-level-exceptional-detection.md)
> confirms the exceptional predicate at the circuit level over real coordinates
> (the Tier B ladder [#4](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/4)
> and the quantum-addend testbed [#27](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/27)
> this needed have both landed).

> **The zero-window ∞ term is removed by an offset encoding** (issue #5 part (b),
> [ADR 0015](adr/0015-offset-window-encoding.md)). The exact measurement
> (`verify/completeness_collision_rate.py`) found the *dominant* exceptional term
> is not `dx=0` (§4) but the **zero-window ∞** addend — a window digit of `0`
> selecting the `[0]·P = ∞` table entry — at `1/2^w` per addition, `≈2⁻¹¹` over
> the ladder. `verify/offset_window_encoding.py` removes it: shifting every window
> digit `g → g+1` (with one compile-time correction point `[(1+d)S]P`) makes the
> emitted index `g+1 ∈ [1, 2^w]` never `0`, so for `2^w < n` the addend is finite
> at every window. This is verified exhaustively on a real toy curve (never emits
> ∞; still computes `[a]P+[b]Q`), and re-measuring shows the `addend=∞` rate is
> then **exactly 0** with `dx=0` unchanged — so the headline below returns to the
> `dx=0`-limited `≈2⁻²⁵⁰` under an explicit encoding condition.

## 4. The `dx=0` collisions are negligible

An addition adds a *fixed* precomputed classical multiple `M = P[k]`. It hits the
`dx=0` branch (x-coordinates equal) exactly when the accumulator `A` satisfies
`A.x == M.x`, i.e. `A ∈ {M, −M}` (the two points sharing that x). Over the
superposition of scalars the accumulator ranges over ~`n` group points; treating
it as approximately equidistributed, the amplitude with `A ∈ {M, −M}` is

```
P[dx=0 at one addition]  ≈  2 / n  ≈  2⁻²⁵⁵
```

The windowed ladder performs `2n/w − 4 = 28` windowed additions (w=16), so by a
union bound the **total** exceptional amplitude across the whole run is

```
28 · 2/n  ≈  56 / n  ≈  2⁻²⁵⁰   ≪   10⁻²  (Shor's tolerance)
```

— over 240 bits of margin. Doubling (`A == M`) and `P=−Q` (`A == −M`) are the two
sub-cases and are already included in the `A ∈ {M,−M}` count.

> **The union bound above is confirmed by an exact end-to-end computation**
> (issue #28, [ADR 0016](adr/0016-exact-mid-ladder-bound.md)).
> `verify/mid_ladder_bound.py` computes the exact `P[⋃_k A_k]` — the probability
> that *any* addition is exceptional across the real two-scalar ladder — by
> tracking the accumulator's clean (never-yet-exceptional) mass through the whole
> run. On every toy config the exact amplitude is `≤` this union bound. At attack
> parameters an exact convolution is infeasible, so the rigorous end-to-end bound
> is the union upper bound itself — `≈ 2⁻²⁵⁰` under the offset encoding (`≈ 2⁻¹¹`
> if the zero-window `∞` term is not removed), both `≪ 10⁻²`; the toy `exact ≤
> union` results certify it is not loose.

## 5. Conclusion

Combining §3 and §4: after the direct-lookup first window removes the amplitude-1
∞ start, the total amplitude on any exceptional adder input across the full
28-addition ladder is `≈ 2⁻²⁵⁰`, over 240 bits below Shor's ~1% tolerance.
The incomplete affine adder is therefore sufficient for a working attack — no
complete formulas (Path B) are required — matching the standard argument in the
literature. This is what justifies `completeness_overhead = 1.0` in
`analysis/ecdlp_estimate.py`.

## Caveats (what keeps this an argument, not a proof)

- **Equidistribution is no longer load-bearing.** The `~1/n` per-addition rate was
  originally justified by assuming the accumulator's x-coordinate is approximately
  uniform over the superposition. A rigorous treatment would bound the actual
  distribution of partial-scalar multiples
  (or invoke a specific ladder ordering that provably avoids `{M,−M}`), as
  Roetteler et al. discuss. That assumption has since been removed on two fronts:
  [ADR 0008](adr/0008-empirical-completeness-collision-rate.md) /
  `verify/completeness_collision_rate.py` measured the rate exactly and found it is
  `O(1)·2/n` and **insensitive to the accumulator's shape** (holding even 250× from
  uniform), because the addend sweeps the group; and
  [ADR 0016](adr/0016-exact-mid-ladder-bound.md) / `verify/mid_ladder_bound.py`
  computes the
  **exact** end-to-end amplitude by tracking the real clean-mass distribution — no
  equidistribution assumption at all. What remains at 256-bit attack scale is only
  that an exact convolution is infeasible, so the reported number is the analytic
  **union upper bound** (`P[⋃ A_k] ≤ Σ_k P[A_k]`), which the toy `exact ≤ union`
  results certify is tight, not loose.

  > **The scalar/dlog model behind this bound is now confirmed at the circuit level
  > over real coordinate arithmetic** (issue #28, [ADR 0018](adr/0018-circuit-level-exceptional-detection.md)).
  > The exact end-to-end bound ([ADR 0016](adr/0016-exact-mid-ladder-bound.md)) and
  > the offset-encoding pin (ADR 0015) both compute in the dlog model, whose one
  > *curve* assumption is `dx=0 ⇔ acc ≡ ±addend (mod n)`.
  > `src/point_add/ec_exceptional.rs` builds a **reversible** exceptional detector —
  > `dx0 = (x1==x2)`, with `acc=∞` / `addend=∞` as `∞`-sentinel zero-tests, on real
  > `(x,y)` coordinate qubits (no modular inverse) — and simulation-measures it over
  > **every** `(accumulator, addend)` pair of **three** real prime-order toy curves
  > (orders 19/29/41, e.g. `y²=x³+2x+2 / F₁₇`) across window widths `w = 2..5`. The
  > measured real-coordinate verdict equals the scalar predicate
  > `(m==0) ∨ (y==0) ∨ (y≡±m)` on every pair (0 mismatches),
  > driving the ADR 0016 survival recursion with the *circuit-measured* predicate
  > reproduces the scalar-model end-to-end residual (`exact ≤ union`), and the offset
  > encoding emits `addend=∞` **never** on real coordinates. So the equivalence this
  > bound rests on is no longer only a dlog assumption (nor only a Python
  > cross-check): it is confirmed by a reversible circuit over real coordinate
  > arithmetic, exhaustively over the group — removing any doubt that `dx=0` on real
  > coordinates is exactly the `{M,−M}` set that the exact bound (ADR 0016) and the
  > shape-insensitive rate (ADR 0008) are built on.
- **The ∞-removal is circuit-demonstrated, not only structural.** The amplitude-1
  ∞ start is removed by the direct-lookup first window
  ([ADR 0009](adr/0009-direct-lookup-init.md) / `direct_lookup_init.py`), built as an
  actual reversible circuit and verified exhaustively on a toy prime-order curve plus
  a secp256k1 256-bit spot-check; the zero-window ∞ is removed by the offset window
  encoding ([ADR 0015](adr/0015-offset-window-encoding.md)). The Tier B ladder
  ([#4](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/4)) and the
  quantum-addend testbed ([#27](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/27))
  this once depended on have both landed (`ladder_full.rs`, `ladder_stream.rs`), so
  the end-to-end structure is now exercised, not only cited from the paper.
- **Phase, not just value.** §2 shows exceptions also corrupt phase; the bound in
  §4 covers this because a mis-phased state at amplitude ε contributes ≤ ε to the
  failure probability, exactly as a wrong-value state does.
- **The argument is now demonstrated by an actual recovery, not only bounded**
  (issue #46, [ADR 0019](adr/0019-end-to-end-ecdlp-recovery.md)).
  `verify/shor_ecdlp_recovery.py` runs the *full* two-register Shor-ECDLP on toy
  prime-order curves by exact statevector simulation and **recovers the secret
  discrete log `m`** while computing `[a]P+[b]Q` with the **incomplete affine adder
  this circuit implements** (chord-only, `inv(0):=0` misfire) plus the completeness
  handling above (direct-lookup init + offset encoding). The **complete** adder gives
  `P_success = (n−1)/n` exactly (a harness check); the **offset + incomplete** adder
  still recovers the true `m` as the maximum-likelihood dlog on curves of order
  19/29/41, with `P_success` rising toward `(n−1)/n` as the exceptional rate thins
  with `n` — the payload the bound was always for. Dropping the offset encoding
  (standard windowing) feeds the zero-window `∞` sentinel to the chord formula and
  collapses recovery, so §4's `∞`-free encoding condition is shown load-bearing for
  the *attack*, not only for the amplitude figure. This is the executable end-to-end
  complement to the exact bound (§4, ADR 0016) and the reversible detector (ADR 0018):
  the completeness argument now ends in a recovered secret.
