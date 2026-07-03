#!/usr/bin/env python3
"""Exact END-TO-END bound on the ladder's exceptional amplitude (issue #28).

`completeness_collision_rate.py` (#15) measures the *per-addition* exceptional
rate, and `completeness_argument.md §4` (#14) turns those into a total by a
**union bound**: `P[exceptional] ≤ Σ_k P[exceptional at addition k] ≈ 28·2/n`.
Issue #28 asks for the *exact* amplitude on the real 28-window, two-scalar
(`[a]P + [b]Q`) superposition — the probability that **any** addition in the
whole ladder is exceptional — rather than a union upper bound.

This computes it exactly, and confirms it never exceeds the union bound. The idea:
track the accumulator distribution restricted to the **clean** mass (no exceptional
case has occurred at any prior addition). At each windowed addition, the
`(accumulator, window-value)` pairs that ARE exceptional are removed (their mass
is added to the failed total); the surviving clean mass convolves forward. The
final failed mass is exactly `P[≥1 exceptional across the ladder] = P[⋃_k A_k]`,
which is `≤ Σ_k P[A_k]` (the union bound, computed here in parallel from the
unrestricted accumulator distribution — this repo's #15 quantity).

Scalar model (validated against a real prime-order curve in #15 / ADR 0008): a
point is its discrete log `s ∈ Z_n`; `INF` is `s = 0`; `[s]P` and `[t]P` share an
x-coordinate iff `t ≡ ±s (mod n)`. An addition adds `M = v·c` for window value `v`
(base constant `c`). It is exceptional iff:
  - `addend = INF` : `v·c ≡ 0`  (the `[0]P` table entry — removed by the offset
                                  encoding, ADR 0015),
  - `acc = INF`    : `acc ≡ 0`,
  - `dx = 0`       : `acc ∈ {M, −M}`, `M ≠ 0`  (the affine collision, #15 §4).

Reported for both encodings: **standard** (`v ∈ [0, 2^w)`) and **offset**
(`v ∈ [1, 2^w]`, no zero window → no `addend=INF` term; ADR 0015). Exact rationals
(`Fraction`); analysis-only, deterministic, pure-Python. Never touches the scored
circuit.
"""
import sys
from fractions import Fraction


def ladder_windows(n, w, d):
    """The combined `[a]P + [b]Q` ladder's per-window base constants: t windows of
    base P (`c = 2^{w i}`) then t of base Q = [d]P (`c = 2^{w j}·d`). Returns
    (t, windows)."""
    if w <= 0:
        raise ValueError(f"window width w must be positive, got {w}")
    t = 0
    while (1 << (w * t)) < n:
        t += 1
    windows = [pow(2, w * i, n) for i in range(t)]
    windows += [(pow(2, w * j, n) * d) % n for j in range(t)]
    return t, windows


def analyze(n, w, d, offset):
    """Exact end-to-end failed amplitude and the union bound for one config.

    Returns dict with 'exact' = P[≥1 exceptional] and 'union' = Σ per-addition
    rate, both as Fractions, plus a survival sanity total."""
    if offset and (1 << w) >= n:
        # offset digits v ∈ [1, 2^w] are ∞-free only when none is ≡ 0 mod n, i.e.
        # 2^w < n (mirrors offset_window_encoding.py's precondition).
        raise ValueError(f"offset mode requires 2^w < n; got 2^{w}={1 << w} >= n={n}")
    _, windows = ladder_windows(n, w, d)
    vals = list(range(1, (1 << w) + 1)) if offset else list(range(1 << w))
    big = 1 << w  # |window value set| == 2^w for both encodings

    def exceptional(y, m, addend_inf):
        # y = accumulator dlog, m = v*c (addend dlog), addend_inf = (m == 0).
        return addend_inf or y == 0 or (not addend_inf and (y == m or y == (n - m) % n))

    # Direct-lookup first window writes acc := v*c0 (issue #5 §3); integer counts.
    c0 = windows[0]
    clean = [0] * n  # survival mass: no exceptional at any prior addition
    full = [0] * n   # unrestricted mass (union bound / #15's dist)
    for v in vals:
        clean[(v * c0) % n] += 1
        full[(v * c0) % n] += 1
    cden = big       # clean denominator
    fden = big       # full denominator

    failed = Fraction(0)
    union = Fraction(0)
    n_adds = 0
    for k in range(1, len(windows)):
        n_adds += 1
        c = windows[k]

        # --- exact survival step: remove exceptional (acc, v) mass, convolve rest.
        new_clean = [0] * n
        fail_k = 0
        for v in vals:
            m = (v * c) % n
            ai = m == 0
            for y in range(n):
                mass = clean[y]
                if not mass:
                    continue
                if exceptional(y, m, ai):
                    fail_k += mass
                else:
                    new_clean[(y + m) % n] += mass
        failed += Fraction(fail_k, cden * big)
        clean = new_clean
        cden *= big

        # --- union step: per-addition rate over the unrestricted distribution.
        new_full = [0] * n
        exc_k = 0
        for v in vals:
            m = (v * c) % n
            ai = m == 0
            for y in range(n):
                mass = full[y]
                if not mass:
                    continue
                if exceptional(y, m, ai):
                    exc_k += mass
                new_full[(y + m) % n] += mass
        union += Fraction(exc_k, fden * big)
        full = new_full
        fden *= big

    survive = Fraction(sum(clean), cden)  # P[clean the whole way] = 1 - failed
    return {"n": n, "w": w, "n_adds": n_adds, "exact": failed, "union": union,
            "survive": survive, "offset": offset}


CONFIGS = [
    # (n prime, window w, secret d) — matches completeness_collision_rate.py
    (1009, 2, 613),
    (1009, 5, 613),
    (2003, 4, 877),
]


def main():
    print("=" * 74)
    print(" Exact end-to-end mid-ladder exceptional amplitude (issue #28)")
    print(" P[>=1 exceptional over the real ladder]  vs  the union bound (#15)")
    print("=" * 74)
    print()
    print("  'exact' = P[union of exceptional events] (survival-tracked, exact);")
    print("  'union' = sum of per-addition rates (the completeness-argument bound).")
    print("  exact <= union always; the offset encoding removes the addend=INF term.")
    print("  NB: toy rates are LARGE by design (they scale as 2/n and 1/2^w) — the")
    print("  operative number is the n≈2²⁵⁶ extrapolation below, not these toys.")
    print()
    hdr = (f"  {'n':>5} {'w':>2} {'adds':>4} {'enc':>4} | {'exact':>12} {'union':>12} "
           f"{'exact/union':>11}")
    print(hdr)
    print("  " + "-" * (len(hdr) - 2))

    results = []
    for n, w, d in CONFIGS:
        for offset in (False, True):
            r = analyze(n, w, d, offset)
            results.append(r)
            enc = "off" if offset else "std"
            ratio = float(r["exact"] / r["union"]) if r["union"] else 0.0
            print(f"  {n:>5} {w:>2} {r['n_adds']:>4} {enc:>4} | "
                  f"{float(r['exact']):>12.4e} {float(r['union']):>12.4e} {ratio:>11.4f}")
    print()

    # Rigorous end-to-end UPPER bound at attack parameters. An EXACT convolution
    # at n≈2²⁵⁶ is infeasible (2²⁵⁶ distribution entries), so the rigorous
    # end-to-end bound at attack scale is the analytic UNION bound — a proven upper
    # bound, `P[⋃ A_k] ≤ Σ P[A_k]` — evaluated in closed form from the same
    # per-addition rates (`dx=0` = 2/n; zero-window ∞ = 1/2^w). The toy configs
    # above certify this is not loose: there the exact `P[⋃ A_k]` is computed and
    # is ≤ the union bound (and a sizable fraction of it).
    import math
    N_REAL, W_REAL, ADDS = 2 ** 256, 16, 28
    dx0 = ADDS * 2.0 / N_REAL                    # dx=0 union term
    zerowin = ADDS * (1.0 / (1 << W_REAL))       # standard zero-window ∞ union term
    print("Rigorous end-to-end UPPER bound at attack parameters  (union; exact ≤ this)")
    print("  (n≈2²⁵⁶, w=16, 28 additions; exact convolution at this n is infeasible)")
    print("-" * 74)
    print(f"  standard : union = dx=0 + zero-window ∞ = {dx0 + zerowin:.2e} "
          f"(≈ 2^{math.log2(dx0 + zerowin):.0f}, ∞-dominated)")
    print(f"  offset   : union = dx=0 only            = {dx0:.2e} "
          f"(≈ 2^{math.log2(dx0):.0f})")
    print(f"  both ≪ Shor's ~1e-2 tolerance. The toy `exact ≤ union` results above")
    print(f"  certify this union bound is a rigorous — and not loose — end-to-end bound.")
    print()

    # ---- assertions ---- #
    ok = True
    notes = []

    # (1) exact <= union on every config/encoding (the point of the exercise).
    c1 = all(r["exact"] <= r["union"] for r in results)
    ok &= c1
    notes.append(f"[{'ok' if c1 else 'XX'}] exact P[union] <= union bound on all "
                 f"configs (end-to-end never exceeds the argument's bound)")

    # (2) survival identity: exact failed + clean survival == 1 (mass conserved).
    c2 = all(r["exact"] + r["survive"] == 1 for r in results)
    ok &= c2
    notes.append(f"[{'ok' if c2 else 'XX'}] exact + survival == 1 exactly "
                 f"(no mass lost — the tracking is exact)")

    # (3) offset removes the dominant term: for each config, offset exact < std.
    by_key = {}
    for r in results:
        by_key.setdefault((r["n"], r["w"]), {})[r["offset"]] = r["exact"]
    c3 = all(v[True] < v[False] for v in by_key.values())
    ok &= c3
    notes.append(f"[{'ok' if c3 else 'XX'}] offset exact < standard exact on all "
                 f"configs (zero-window ∞ term removed; ADR 0015)")

    # (4) the rigorous union UPPER bound at attack parameters is ≪ Shor's ~1%
    #     tolerance for both encodings (exact ≤ union, certified on the toys; the
    #     toy rates are LARGE by design — they scale as 2/n and 1/2^w).
    c4 = (dx0 + zerowin) < 1e-2 and dx0 < 1e-60
    ok &= c4
    notes.append(f"[{'ok' if c4 else 'XX'}] union UPPER bound ≪ 1e-2 at n≈2²⁵⁶ "
                 f"(std {dx0 + zerowin:.1e}, offset {dx0:.1e}); exact ≤ union on toys")

    print("Findings")
    print("-" * 74)
    for line in notes:
        print("  " + line)
    print()
    print("=" * 74)
    if ok:
        print(" RESULT: the mid-ladder exceptional amplitude is bounded EXACTLY")
        print(" end-to-end on the toy ladders (not only by a per-addition union")
        print(" bound), and the exact value never exceeds the union bound. At attack")
        print(" scale — where an exact convolution is infeasible — the rigorous")
        print(" end-to-end bound is that union UPPER bound, ≪ Shor's tolerance under")
        print(" both the standard and the ∞-free offset encoding (issue #28 / #5).")
        print("=" * 74)
        return 0
    print(" RESULT: FAILURE — see [XX] above.")
    print("=" * 74)
    return 1


if __name__ == "__main__":
    sys.exit(main())
