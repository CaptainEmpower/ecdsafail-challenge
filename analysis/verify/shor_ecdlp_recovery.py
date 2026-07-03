#!/usr/bin/env python3
"""End-to-end Shor-ECDLP: actually recover the secret discrete log on toy curves,
using the *incomplete affine adder this circuit implements* plus the repo's
completeness handling (issue #46, ADR 0019).

Every completeness result so far stops one step short of the payload:
`completeness_collision_rate.py` (#5) and `mid_ladder_bound.py` (#28) *bound* the
exceptional amplitude; `src/point_add/ec_exceptional.rs` (#28, ADR 0018) *detects*
the exceptional set as a reversible circuit. None of them shows the thing an
attacker wants — that the full Shor-ECDLP pipeline, driven by the incomplete
chord-only adder with our exceptional-case handling, **recovers the secret m from
Q = [m]P**. That is the repo's sharpest honest non-claim ("no demonstrated
end-to-end attack"). This module closes it at toy scale, by EXACT statevector
simulation (no Monte-Carlo).

The pipeline (standard two-register Shor-ECDLP over a prime-order group Z_n):

  1. |a>|b> uniform over Z_n x Z_n.
  2. Oracle writes the point register  |a>|b>|O>  ->  |a>|b>|[a]P + [b]Q>,
     computed by the windowed affine ladder (direct-lookup init writes the
     accumulator; each later window adds a precomputed multiple).
  3. QFT_n on both index registers, then the EXACT Born-rule distribution
        P(c,d) = sum_R | (1/n^2) sum_{(a,b): R(a,b)=R} w^{ca+db} |^2 ,  w=e^{2pi i/n}.
  4. Classical post-processing: a measured (c,d) with c != 0 (always invertible mod
     the prime n) yields  m = d * c^{-1} (mod n);  success <=> d == c*m (mod n).

The point of the demonstration is to run step 2 with the INCOMPLETE adder, three
ways, and show recovery survives the exceptional cases the way the completeness
argument predicts:

  - complete            reference group law  ->  P_success = (n-1)/n exactly
                        (this validates the statevector harness);
  - offset  + incomplete   chord-only adder (inv(0):=0 misfire), OFFSET digit set
                        (g->g+1, ADR 0015) so the addend is never the inf sentinel
                        and direct-lookup keeps acc finite -> only rare dx=0 misfires
                        -> STILL recovers m;
  - standard + incomplete  same adder, STANDARD digit set, where a zero window
                        selects the [0]P = inf sentinel (0,0) and feeds it to the
                        chord formula -> corrupts recovery.

So the offset encoding's value is shown for the *attack* (recovery probability),
not only for the amplitude bound. The count of corrupted (a,b) basis states is
cross-checked against the exact exceptional rate of `completeness_collision_rate`.

The `inv(0):=0` model. The scored point-add uses a chord/tangent affine addition;
on dx=0 its modular-inverse step has no inverse. Taking the slope as
`lambda = (y2-y1)*inv(dx)` with `inv(0):=0` makes the adder EXACTLY the group law
when dx!=0 and a deterministic wrong point when dx=0 — a faithful stand-in for
"the affine adder corrupts the output on the exceptional state". The inf addend is
the same (0,0) sentinel the circuit uses and is NOT special-cased, so the standard
encoding reproduces the zero-window corruption the offset encoding removes.

Analysis-only, deterministic, pure-Python (cmath for the QFT phases; no numpy /
no z3). Reuses the toy-curve group law + finder validated in #5/#15. Never touches
the scored circuit. Toy scale by construction (exact P(c,d) is O(n^4)); the 256-bit
attack stays out of reach — that is what `ecdlp_estimate.py` is for.
"""
import cmath
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from completeness_collision_rate import (  # noqa: E402
    INF,
    Curve,
    _is_prime,
    measure_ladder,
)


# --------------------------------------------------------------------------- #
# Toy curves: small prime-order groups (so <G> ~ Z_n and every point has a dlog).
# --------------------------------------------------------------------------- #

def find_small_prime_order_curves(orders):
    """Deterministically find one prime-order curve per requested group order.

    Searches a fixed grid of p (kept p % 4 == 3 for a trivial sqrt in points()),
    a, b; returns {n: (curve, generator)}. Raises if any order is unmet so a
    silently-missing curve fails loudly rather than skipping a config."""
    want = set(orders)
    found = {}
    for p in range(7, 200):
        if not _is_prime(p) or p % 4 != 3:
            continue
        for a in range(0, 5):
            for b in range(1, 7):
                if (4 * a ** 3 + 27 * b ** 2) % p == 0:
                    continue
                c = Curve(p, a, b)
                n = len(c.points())  # includes INF -> full group order
                if n in want and n not in found and _is_prime(n):
                    c.order = n
                    gen = next(pt for pt in c.points() if pt is not INF)
                    # validate: prime order, generator on-curve and non-identity
                    if gen is INF or not c.is_on(gen):
                        continue
                    found[n] = (c, gen)
        if want <= set(found):
            break
    missing = want - set(found)
    if missing:
        raise RuntimeError(f"no prime-order toy curve found for orders {sorted(missing)}")
    return found


# --------------------------------------------------------------------------- #
# The incomplete affine adder (chord-only, inv(0):=0) — exactly the circuit's
# exceptional behaviour.  Points are (x, y) tuples; the point at infinity is the
# (0, 0) sentinel (never an on-curve point here, since every toy curve has b >= 1).
# --------------------------------------------------------------------------- #

INF_SENT = (0, 0)  # affine sentinel for the point at infinity, as the circuit uses


def incomplete_add(p, P, Q):
    """Chord-only affine addition with inv(0):=0.

    Equals the group law when dx != 0; on dx == 0 (doubling, P == -Q, or an inf
    sentinel that happens to share x) it MISFIRES to a deterministic wrong point —
    the faithful model of the incomplete adder. inf is NOT special-cased, so a
    (0,0)-sentinel addend/accumulator is fed straight into the formula (that is the
    zero-window corruption the offset encoding removes)."""
    x1, y1 = P
    x2, y2 = Q
    dx = (x2 - x1) % p
    inv = pow(dx, -1, p) if dx else 0  # circuit's inv(0):=0 misfire convention
    lam = ((y2 - y1) * inv) % p
    x3 = (lam * lam - x1 - x2) % p
    y3 = (lam * (x1 - x3) - y1) % p
    return (x3, y3)


def _digits(scalar, w, t):
    mask = (1 << w) - 1
    return [(scalar >> (w * i)) & mask for i in range(t)]


def window_count(n, w):
    t = 0
    while (1 << (w * t)) < n:
        t += 1
    return t


def build_oracle(curve, gen, n, w, d, encoding, adder):
    """Return R_key[a][b] for every (a, b) in Z_n x Z_n, where R(a,b)=[a]P+[b]Q is
    computed by the windowed affine ladder.

      encoding: "standard" (digit g) or "offset" (digit g+1, ADR 0015 — addend
                never the inf sentinel, acc never starts inf);
      adder:    "complete" (reference group law) or "incomplete" (chord-only,
                inv(0):=0 misfire — exactly the circuit's exceptional behaviour).

    R identities are hashable keys used only to group (a,b) with equal R. Both
    adders emit the (x,y) tuple (the identity as the (0,0) sentinel), so a
    same-encoding complete-vs-incomplete comparison isolates exceptional misfires.

    The three headline MODELS are:
      complete = (standard, complete) -> ideal [a+d b]P, P_success=(n-1)/n;
      offset   = (offset,   incomplete);
      standard = (standard, incomplete)."""
    t = window_count(n, w)
    p = curve.p
    off = 1 if encoding == "offset" else 0

    # dlog tables over the REAL curve.  mult[s] = [s]P; sentinel form maps INF->(0,0).
    mult = [curve.mul(s, gen) for s in range(n)]
    assert mult[0] is INF and all(pt is not INF for pt in mult[1:])
    Qbase = curve.mul(d, gen)  # Q = [d]P
    multQ = [curve.mul(s, Qbase) for s in range(n)]
    assert multQ[0] is INF
    sent = [INF_SENT if s == 0 else mult[s] for s in range(n)]
    sentQ = [INF_SENT if s == 0 else multQ[s] for s in range(n)]
    tabP, tabQ = (sent, sentQ) if adder == "incomplete" else (mult, multQ)

    pow2 = [pow(2, w * i, n) for i in range(t)]

    def scalars_for(a, b):
        # addend scalar per window: base P (multiplier 2^{wi}) then base Q.
        sc = [((g + off) * pow2[i]) % n for i, g in enumerate(_digits(a, w, t))]
        sc += [((h + off) * pow2[j]) % n for j, h in enumerate(_digits(b, w, t))]
        return sc

    if adder == "complete":
        def step(acc, addend):
            return curve.add(acc, addend)
    else:
        def step(acc, addend):
            return incomplete_add(p, acc, addend)

    def R(a, b):
        sc = scalars_for(a, b)
        acc = _lookup(tabP, tabQ, sc, 0, t)           # direct-lookup init (writes acc)
        for k in range(1, len(sc)):
            acc = step(acc, _lookup(tabP, tabQ, sc, k, t))
        return INF_SENT if acc is INF else acc        # normalize identity key

    keys = [[None] * n for _ in range(n)]
    for a in range(n):
        row = keys[a]
        for b in range(n):
            row[b] = R(a, b)
    return keys


# The three headline models -> (encoding, adder).
MODELS = {
    "complete": ("standard", "complete"),
    "offset": ("offset", "incomplete"),
    "standard": ("standard", "incomplete"),
}


def oracle_for(curve, gen, n, w, d, mode):
    enc, add = MODELS[mode]
    return build_oracle(curve, gen, n, w, d, enc, add)


def _lookup(tabP, tabQ, sc, k, t):
    """Window k selects from the base-P table for k < t, else the base-Q table."""
    if k < t:
        return tabP[sc[k]]
    return tabQ[sc[k]]


# --------------------------------------------------------------------------- #
# Exact statevector distribution and the classical recovery.
# --------------------------------------------------------------------------- #

def _P_at(c, d_reg, keys, n, wpow):
    """Exact P(c, d_reg) = (1/n^4) sum_R | sum_{(a,b):R} w^{ca+d_reg b} |^2."""
    buckets = {}
    for a in range(n):
        ca = (c * a) % n
        row = keys[a]
        for b in range(n):
            ph = wpow[(ca + d_reg * b) % n]
            k = row[b]
            v = buckets.get(k)
            buckets[k] = ph if v is None else v + ph
    s = 0.0
    for v in buckets.values():
        s += v.real * v.real + v.imag * v.imag
    return s / (n ** 4)


def full_grid(keys, n, wpow):
    return [[_P_at(c, d, keys, n, wpow) for d in range(n)] for c in range(n)]


def recover_secret(grid, n):
    """Argmax over candidate secrets m' of the recoverable line mass
    sum_{c!=0} P(c, c*m').  Every (c,d) with c!=0 lies on exactly one m'-line
    (m' = d*c^{-1}), so this is the maximum-likelihood dlog from the distribution."""
    best_m, best_mass = None, -1.0
    for mp in range(n):
        mass = sum(grid[c][(c * mp) % n] for c in range(1, n))
        if mass > best_mass:
            best_m, best_mass = mp, mass
    return best_m, best_mass


def corrupted_fraction(keys_inc, keys_ref, n):
    """Fraction of (a,b) where the incomplete ladder's R differs from the SAME-encoding
    complete ladder's R — i.e. an exceptional addition misfired. Both oracles must use
    the same digit set so the only difference is the misfire (not the offset constant)."""
    diff = 0
    for a in range(n):
        for b in range(n):
            if keys_inc[a][b] != keys_ref[a][b]:
                diff += 1
    return diff / (n * n)


# --------------------------------------------------------------------------- #

# (order n, window w, secret m).  Small so the exact O(n^4) grid is fast; w=2 gives
# several windowed additions per base, and 2^w < n keeps the offset encoding inf-free.
CONFIGS = [
    (19, 2, 7),
    (29, 2, 11),
    (41, 2, 17),
]


def main():
    print("=" * 74)
    print(" End-to-end Shor-ECDLP: recover the discrete log on toy curves, using")
    print(" the incomplete affine adder + offset/direct-lookup handling (issue #46)")
    print("=" * 74)
    print()

    curves = find_small_prime_order_curves({n for n, _, _ in CONFIGS})
    ok = True

    # --------------------------------------------------------------------- #
    # Part A — a concrete recovered secret + normalization check (smallest n),
    #          for all three adder models, via the full exact P(c,d) grid.
    # --------------------------------------------------------------------- #
    n0, w0, m0 = CONFIGS[0]
    curve0, gen0 = curves[n0]
    wpow0 = [cmath.exp(2j * math.pi * k / n0) for k in range(n0)]
    print(f"Part A — full exact distribution on the smallest curve "
          f"(order n={n0}, w={w0}, secret m={m0})")
    print("-" * 74)
    print(f"  curve y^2 = x^3 + {curve0.a}x + {curve0.b} over F_{curve0.p}, "
          f"prime order n={n0}, generator P={gen0}, Q=[m]P")
    print()
    print(f"  {'adder model':<22} {'norm':>10} {'P(c=0)':>9} {'P_success':>10} "
          f"{'recovered m':>12}")
    print("  " + "-" * 68)
    a_results = {}
    for mode in ("complete", "offset", "standard"):
        keys = oracle_for(curve0, gen0, n0, w0, m0, mode)
        grid = full_grid(keys, n0, wpow0)
        norm = sum(sum(row) for row in grid)
        pc0 = sum(grid[0][d] for d in range(n0))
        psucc = sum(grid[c][(c * m0) % n0] for c in range(1, n0))
        rec_m, _ = recover_secret(grid, n0)
        a_results[mode] = dict(norm=norm, pc0=pc0, psucc=psucc, rec_m=rec_m, keys=keys)
        flag = "OK" if rec_m == m0 else "**"
        print(f"  {mode:<22} {norm:>10.6f} {pc0:>9.4f} {psucc:>10.4f} "
              f"{rec_m:>10} {flag}")
    print()

    # locks for Part A
    c_complete = a_results["complete"]
    exact_complete = (n0 - 1) / n0
    a1 = abs(c_complete["psucc"] - exact_complete) < 1e-9 and c_complete["rec_m"] == m0
    ok &= a1
    print(f"  [{'ok' if a1 else 'XX'}] complete adder: P_success == (n-1)/n = "
          f"{exact_complete:.6f} exactly, and recovers m (harness validated)")

    a2 = a_results["offset"]["rec_m"] == m0
    ok &= a2
    print(f"  [{'ok' if a2 else 'XX'}] offset + INCOMPLETE adder: recovers the true "
          f"secret m={m0} (exceptions survived)")

    a3 = a_results["offset"]["psucc"] > a_results["standard"]["psucc"] + 1e-9
    ok &= a3
    print(f"  [{'ok' if a3 else 'XX'}] offset P_success ({a_results['offset']['psucc']:.4f}) "
          f"> standard ({a_results['standard']['psucc']:.4f}) — the zero-window inf "
          f"term degrades the ATTACK, not just the bound")

    a4 = all(abs(a_results[mode]["norm"] - 1.0) < 1e-6 for mode in a_results)
    ok &= a4
    print(f"  [{'ok' if a4 else 'XX'}] all distributions normalize to 1 "
          f"(exact statevector, unitary QFT)")
    print()

    # --------------------------------------------------------------------- #
    # Part B — recovered secret across configs, via the AUTHORITATIVE argmax over
    # every candidate m' (the maximum-likelihood dlog from the exact distribution).
    # Shows offset P_success -> (n-1)/n as exceptions thin with n.
    # --------------------------------------------------------------------- #
    print("Part B — recovered secret across toy configs (max-likelihood argmax)")
    print("-" * 74)
    print("  Every (c,d) with c!=0 lies on exactly one m'-line (m'=d c^{-1}); the argmax")
    print("  over m' of the line mass is the maximum-likelihood dlog. 'P_s' is that mass")
    print("  for the TRUE m. offset & complete recover m; offset P_s rises toward (n-1)/n")
    print("  as exceptions thin with n; standard is wrecked by the zero-window inf.")
    print()
    hdr = (f"  {'n':>4} {'w':>2} | {'cmpl P_s':>9} {'off P_s':>9} {'std P_s':>9} "
           f"| {'cmpl m':>7} {'off m':>6} {'true m':>7}")
    print(hdr)
    print("  " + "-" * (len(hdr) - 2))

    for n, w, m in CONFIGS:
        curve, gen = curves[n]
        wpow = [cmath.exp(2j * math.pi * k / n) for k in range(n)]
        ps, rec = {}, {}
        for mode in ("complete", "offset", "standard"):
            if n == n0:  # reuse Part A's full-grid results for the smallest curve
                ps[mode] = a_results[mode]["psucc"]
                rec[mode] = a_results[mode]["rec_m"]
                continue
            keys = oracle_for(curve, gen, n, w, m, mode)
            grid = full_grid(keys, n, wpow)
            ps[mode] = sum(grid[c][(c * m) % n] for c in range(1, n))
            rec[mode], _ = recover_secret(grid, n)
        print(f"  {n:>4} {w:>2} | {ps['complete']:>9.4f} {ps['offset']:>9.4f} "
              f"{ps['standard']:>9.4f} | {rec['complete']:>7} {rec['offset']:>6} "
              f"{m:>7}")

        b1 = abs(ps["complete"] - (n - 1) / n) < 1e-9 and rec["complete"] == m
        b2 = rec["offset"] == m                        # offset recovers the true secret
        b3 = ps["offset"] > ps["standard"] + 1e-9      # offset beats standard
        ok &= b1 and b2 and b3
        if not (b1 and b2 and b3):
            print(f"       [XX] n={n}: complete-exact={b1} offset-recovers={b2} "
                  f"offset>standard={b3}")
    print()

    # --------------------------------------------------------------------- #
    # Part C — cross-check: corrupted basis-state fraction vs the measured
    # exceptional rate of the ladder (completeness_collision_rate).
    # --------------------------------------------------------------------- #
    print("Part C — corrupted (a,b) fraction vs the measured ladder exceptional rate")
    print("-" * 74)
    print("  The offset ladder's only exceptions are dx=0 misfires; the fraction of")
    print("  (a,b) it corrupts should sit near the per-run exceptional probability the")
    print("  exact-distribution measurement (#5) predicts — same order, not identical")
    print("  (registers here are uniform on [0,n), not [0,2^m)).")
    print()
    # same-encoding complete references, so the diff is purely the misfires.
    off_ref = build_oracle(curve0, gen0, n0, w0, m0, "offset", "complete")
    std_ref = build_oracle(curve0, gen0, n0, w0, m0, "standard", "complete")
    off_corrupt = corrupted_fraction(a_results["offset"]["keys"], off_ref, n0)
    std_corrupt = corrupted_fraction(a_results["standard"]["keys"], std_ref, n0)
    meas = measure_ladder(n0, w0, m0, f"n={n0}")
    # union-bound exceptional probability over the run (offset removes addend=inf)
    off_pred = float(meas["dx0"])          # dx=0 mass only (offset is inf-free)
    std_pred = float(meas["any"])          # includes the zero-window inf term
    print(f"  offset:   corrupted (a,b) = {off_corrupt:6.3f}   "
          f"~ measured dx=0 union {off_pred:6.3f}")
    print(f"  standard: corrupted (a,b) = {std_corrupt:6.3f}   "
          f"~ measured total union {std_pred:6.3f}  (zero-window inf included)")
    c1 = std_corrupt > off_corrupt
    ok &= c1
    print(f"  [{'ok' if c1 else 'XX'}] standard corrupts more than offset "
          f"(the inf term the offset encoding removes)")
    print()

    print("=" * 74)
    if ok:
        print(" RESULT: the full Shor-ECDLP pipeline RECOVERS the secret discrete log")
        print(" on toy curves, using the incomplete affine adder our circuit implements")
        print(" plus offset-window + direct-lookup handling. The complete adder gives")
        print(" P_success=(n-1)/n exactly; the offset+incomplete adder still recovers m;")
        print(" the standard encoding's zero-window inf degrades the attack — the")
        print(" demonstrated-attack complement to the amplitude bound (issue #46, ADR 0019).")
        print("=" * 74)
        return 0
    print(" RESULT: FAILURE — a locked expectation regressed (see [XX] above).")
    print("=" * 74)
    return 1


if __name__ == "__main__":
    sys.exit(main())
