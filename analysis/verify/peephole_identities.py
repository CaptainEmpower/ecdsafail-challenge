#!/usr/bin/env python3
"""Formal (z3/SMT) proofs of the boolean-logic invariants the peephole and
uncompute optimizations rely on.

These are the claims currently checked only *empirically* (CONSTPROP_VERIFY,
ALT_SEED_*): re-running the op-stream over sampled shots and asserting the
transform preserved the state. Here we discharge them as universally-quantified
theorems: z3 returns `unsat` on the negation, i.e. NO assignment violates the
identity -> proven for all inputs, not just the sampled ones.

Sources in the repo:
  - constprop.rs (DropZeroCtrl / FoldCx / FoldX / FoldEqualCtrls /
    DropComplementCtrls / InversePairCancellation)
  - venting.rs, arith/adder.rs (Cuccaro / HRS carry-xor recurrence)
  - trailmix_ludicrous/comparator.rs (a<b flag via carry chain)
"""
import sys
from z3 import (BitVec, BitVecVal, Solver, Extract, Concat, If, Not, And, ULT,
                unsat, sat)

results = []


def prove(name, claim_solver_setup):
    """claim_solver_setup(s) must add the NEGATION of the claim to solver s.
    unsat => claim holds for all inputs."""
    s = Solver()
    claim_solver_setup(s)
    r = s.check()
    ok = (r == unsat)
    results.append((name, ok, r))
    status = "PROVED " if ok else "FAILED "
    print(f"  [{status}] {name}" + ("" if ok else f"  (z3: {r}; counterexample: {s.model()})"))
    return ok


bit = lambda nm: BitVec(nm, 1)
ONE = BitVecVal(1, 1)
ZERO = BitVecVal(0, 1)
# CCX action on target: t' = t XOR (a AND b)
ccx = lambda a, b, t: t ^ (a & b)

print("== CCX peephole identities (constprop.rs) ==")

# 1.1 DropZeroCtrl: control a==0  =>  CCX is identity on target
prove("1.1 DropZeroCtrl:  a=0  =>  CCX(a,b,t) == t", lambda s: (
    s.add(bit('a') == ZERO),
    s.add(ccx(bit('a'), bit('b'), bit('t')) != bit('t')),
))

# 1.2 FoldCx: control a==1  =>  CCX(a,b,t) == CX(b,t) == t XOR b
prove("1.2 FoldCx:       a=1  =>  CCX(a,b,t) == t XOR b", lambda s: (
    s.add(bit('a') == ONE),
    s.add(ccx(bit('a'), bit('b'), bit('t')) != (bit('t') ^ bit('b'))),
))

# 1.3 FoldX: a==1 and b==1  =>  CCX == X(t) == t XOR 1
prove("1.3 FoldX:        a=1,b=1 => CCX(a,b,t) == NOT t", lambda s: (
    s.add(bit('a') == ONE, bit('b') == ONE),
    s.add(ccx(bit('a'), bit('b'), bit('t')) != (bit('t') ^ ONE)),
))

# 1.4 FoldEqualCtrls: affine analysis proves a==b on every shot => CCX == CX(a,t)
prove("1.4 FoldEqualCtrls:      a==b  =>  CCX(a,b,t) == t XOR a", lambda s: (
    s.add(bit('a') == bit('b')),
    s.add(ccx(bit('a'), bit('b'), bit('t')) != (bit('t') ^ bit('a'))),
))

# 1.5 DropComplementCtrls: a == NOT b on every shot => CCX is a no-op
prove("1.5 DropComplementCtrls: a==~b =>  CCX(a,b,t) == t", lambda s: (
    s.add(bit('a') == (bit('b') ^ ONE)),
    s.add(ccx(bit('a'), bit('b'), bit('t')) != bit('t')),
))

# 2.1 InversePairCancellation: two identical CCX with controls+target untouched
#     between them cancel. v = a AND b applied twice: (t^v)^v == t.
prove("2.1 InversePairCancellation: CCX;CCX (ctrls/target unchanged) == identity", lambda s: (
    s.add(ccx(bit('a'), bit('b'), ccx(bit('a'), bit('b'), bit('t'))) != bit('t')),
))

print("\n== Ripple-carry adder recurrence (Cuccaro / HRS carry-xor, venting.rs) ==")


def prove_adder(w):
    """Prove the carry recurrence used by the vented/HRS/Cuccaro adders computes
    exact integer addition mod 2^w, for a symbolic w-bit a,b."""
    def setup(s):
        a = BitVec(f'a{w}', w)
        b = BitVec(f'b{w}', w)
        # bit-serial ripple: carry c[0]=0; sum[i]=a[i]^b[i]^c[i];
        # c[i+1] = majority(a[i],b[i],c[i]) = (a&b)|(c&(a^b))
        carry = ZERO
        sum_bits = []
        for i in range(w):
            ai = Extract(i, i, a)
            bi = Extract(i, i, b)
            si = ai ^ bi ^ carry
            sum_bits.append(si)
            carry = (ai & bi) | (carry & (ai ^ bi))
        # assemble little-endian
        ssum = sum_bits[0]
        for i in range(1, w):
            ssum = Concat(sum_bits[i], ssum)
        s.add(ssum != (a + b))  # a+b already truncates to w bits (mod 2^w)
    return prove(f"ripple-carry recurrence == (a+b) mod 2^{w}", setup)


# Small widths exercise the recurrence shape; 256/257 are the PRODUCTION widths
# (256-bit coordinate registers and the 257-bit Solinas extended register), so
# the adder is proved AT the width the scored circuit runs, not extrapolated to
# it. z3 discharges each in <0.2 s (ripple structure bit-blasts cheaply).
for w in (1, 2, 3, 4, 8, 16, 32, 64, 256, 257):
    prove_adder(w)

print("\n== Less-than comparator via borrow chain (comparator.rs) ==")


def prove_cmp(w):
    """flag := (a < b) computed by the subtract-borrow chain equals ULT(a,b)."""
    def setup(s):
        a = BitVec(f'ca{w}', w)
        b = BitVec(f'cb{w}', w)
        # a < b  iff  a - b borrows out of the top bit.
        # borrow[0]=0; borrow[i+1] = (~a[i] & b[i]) | (borrow[i] & ~(a[i]^b[i]))
        borrow = ZERO
        for i in range(w):
            ai = Extract(i, i, a)
            bi = Extract(i, i, b)
            borrow = ((~ai) & bi) | (borrow & (~(ai ^ bi)))
        flag_lt = borrow  # final borrow-out == (a < b)
        s.add(flag_lt != If(ULT(a, b), ONE, ZERO))
    return prove(f"borrow-chain flag == (a <_u b), width {w}", setup)


# Same rationale as the adder: 256/257 are the production comparator widths.
for w in (1, 2, 3, 4, 8, 16, 32, 64, 256, 257):
    prove_cmp(w)


print("\n== Affine-form analysis soundness (constprop.rs premise) ==")
# The CCX peephole identities above are sound GIVEN a premise the analysis SUPPLIES:
# that two controls it marks equal / complementary / constant really are so on every
# basis state. `constprop.rs` tracks each qubit as a GF(2)-linear (affine) form over
# "fresh" input variables — a characteristic vector `p` (which vars are XORed) plus a
# const bit `c` — evaluating to
#       eval(p, c; x) = c XOR (XOR_i  p_i AND x_i)
# and combines forms with `xor_set` (symmetric difference == XOR of characteristic
# vectors). These lemmas discharge the premise at the domain level; the REAL `xor_set`
# is bound to "symmetric difference over a canonical (sorted, de-duped) form" by the
# exhaustive Rust test `constprop::affine_soundness::xor_set_is_symmetric_difference_and_canonical`
# (`cargo test`). Together: the tracker's equal/complement/constant claims hold on
# every basis state — the premise the 26 identities assume, previously only argued
# ("standard linearity argument") and sampled (`CONSTPROP_VERIFY`).


def _affine_eval(p, c, x, n):
    """c XOR parity(p & x) — evaluate an affine form (char-vector p, const c) at x."""
    pv = p & x
    par = ZERO
    for i in range(n):
        par = par ^ Extract(i, i, pv)
    return c ^ par


def prove_affine_atom():
    """The per-POSITION core of XOR-linearity: `(a⊕b)∧x == (a∧x)⊕(b∧x)` on single bits.

    `eval` is `c ⊕ ⊕_i (p_i ∧ x_i)` — a sum over independent positions — so its
    XOR-linearity decomposes position-by-position into exactly this 1-bit distributive
    identity. Proving it here establishes linearity at **every** width N (incl. the
    production 512-variable universe) without a width-512 solve, which is z3-pathological
    (parity of an AND of two symbolic wide vectors); the concrete `prove_affine_linearity`
    widths below then exercise the full char-vector `eval`."""
    def setup(s):
        a, b, x = BitVec("atoma", 1), BitVec("atomb", 1), BitVec("atomx", 1)
        s.add(((a ^ b) & x) != ((a & x) ^ (b & x)))
    return prove("affine per-position atom: (a⊕b)∧x==(a∧x)⊕(b∧x) ⇒ XOR-linearity ∀N", setup)


def prove_affine_linearity(n):
    """CX / equal-fold transfer: eval of the xor_set-combined form is the XOR of the
    two forms' evals — i.e. `xor_set` (symmetric difference) is GF(2)-linear on eval."""
    def setup(s):
        pt, pc, x = BitVec(f"lpt{n}", n), BitVec(f"lpc{n}", n), BitVec(f"lx{n}", n)
        ct, cc = BitVec(f"lct{n}", 1), BitVec(f"lcc{n}", 1)
        lhs = _affine_eval(pt ^ pc, ct ^ cc, x, n)
        rhs = _affine_eval(pt, ct, x, n) ^ _affine_eval(pc, cc, x, n)
        s.add(lhs != rhs)
    return prove(f"affine XOR-linearity: eval(a⊕b)==eval(a)⊕eval(b), N={n}", setup)


def prove_affine_equal(n):
    """FoldEqualCtrls premise: equal affine forms ⇒ equal on every basis state."""
    def setup(s):
        pa, pb, x = BitVec(f"epa{n}", n), BitVec(f"epb{n}", n), BitVec(f"ex{n}", n)
        ca, cb = BitVec(f"eca{n}", 1), BitVec(f"ecb{n}", 1)
        s.add(pa == pb, ca == cb)                                  # premise the tracker checks
        s.add(_affine_eval(pa, ca, x, n) != _affine_eval(pb, cb, x, n))
    return prove(f"FoldEqualCtrls premise: set(a)==set(b) ∧ cst(a)==cst(b) ⇒ a==b, N={n}", setup)


def prove_affine_complement(n):
    """DropComplementCtrls premise: same set, differing const ⇒ a == NOT b (so a&b=0)."""
    def setup(s):
        pa, pb, x = BitVec(f"cpa{n}", n), BitVec(f"cpb{n}", n), BitVec(f"cx{n}", n)
        ca, cb = BitVec(f"cca{n}", 1), BitVec(f"ccb{n}", 1)
        s.add(pa == pb, ca != cb)
        s.add((_affine_eval(pa, ca, x, n) ^ _affine_eval(pb, cb, x, n)) != ONE)  # a xor b == 1
    return prove(f"DropComplementCtrls premise: set(a)==set(b) ∧ cst(a)≠cst(b) ⇒ a==¬b, N={n}", setup)


def prove_affine_constant(n):
    """DropZeroCtrl premise: an empty affine set ⇒ the qubit is the constant c."""
    def setup(s):
        c, x = BitVec(f"kc{n}", 1), BitVec(f"kx{n}", n)
        s.add(_affine_eval(BitVecVal(0, n), c, x, n) != c)  # empty set ⇒ eval == c, ∀ x
    return prove(f"DropZeroCtrl premise: empty set ⇒ constant, N={n}", setup)


# XOR-linearity: the width-independent per-position atom (⇒ all N) plus concrete
# char-vector widths. A direct width-512 eval-linearity solve is z3-pathological
# (parity of an AND of two symbolic 512-bit vectors, ~25 s already at 256), and the atom
# proves the same fact for every N, so it is not needed.
prove_affine_atom()
for n in (4, 8, 16, 64):
    prove_affine_linearity(n)

# Equal / complement / constant reference the affine forms directly, so the parity terms
# cancel by congruence and these are cheap even at the PRODUCTION variable universe:
# `constprop` seeds one fresh var per input qubit, and `trailmix_ludicrous` feeds it
# reg0+reg1 = 2×256 = **512** input qubits (a single form's set is capped at CAP_SET=2048).
# Prove them there, not just at small N (matching the referee-F3 "prove at the production
# width" standard used for the adder/comparator above).
for n in (4, 8, 16, 64, 256, 512):
    prove_affine_equal(n)
    prove_affine_complement(n)
    prove_affine_constant(n)

# ---- summary ----
n_ok = sum(1 for _, ok, _ in results if ok)
print(f"\n=== {n_ok}/{len(results)} lemmas PROVED (unsat on negation) ===")
sys.exit(0 if n_ok == len(results) else 1)
