#!/usr/bin/env python3
"""Solinas modular reduction, proved over the **actually emitted** `mod_add_qq`
op-stream — the emitter-bound complement to `solinas_reduction.py`.

`solinas_reduction.py` proves `mod_add_qq` computes `(acc + a) mod p` by mirroring
the algorithm "step-for-step" in z3 BitVec, and `src/kani_proofs.rs::solinas_add` is
a hand-written integer twin (the exact copy referee finding **F2** warns about). Both
are re-implementations: if the gate emitter drifts from the model/twin, they stay
green. ADR 0027/0030 bound the underlying `cuccaro_add_fast` *adder* to the emitter;
this binds the modular-reduction *wrapper* (add `c = 2^256 − p`, branch on the
overflow bit, undo or clear, uncompute the flag) to it as well.

It replays the real emitted op-stream — dumped by the real `B` builder into
`analysis/mod_add_qq_ops.json` (see `src/point_add/modadd_dump.rs`; a `#[cfg(test)]`
drift guard keeps the artifact byte-identical to a fresh emit) — through the
`proof_toolkit` z3 model of `src/sim.rs`, with the `R`-reset measurement outcomes
**free/∀**, and proves, for **all** `acc, a ∈ [0, p)` on the secp256k1 field:

  1. FUNCTIONAL   `acc' == (acc + a) mod p`
  2. A-PRESERVED  `a` unchanged
  3. CLEAN        the flag qubit and every ancilla (const-load regs, carries,
                  ext-overflow bits) return to |0>
  4. PHASE-CLEAN  net phase == 0 for every measurement outcome — each `R` reset the
                  circuit runs when freeing an ancilla is phase-neutral, which holds
                  iff that ancilla is genuinely |0> at reset time

The reference `(acc + a) mod p` is an **independent** ripple-carry-add + conditional-
subtract-p spec (textbook reduce-by-conditional-subtraction), structurally unlike the
Solinas add-`c`-and-check-overflow implementation being replayed. z3 discharges the
whole thing by `unsat` on the negation — a proof over all inputs and all outcomes at
the production 256-bit width, not a sample. Analysis-only; never touches the circuit.
"""
import os
import sys

from z3 import And, Bool, BoolVal, If, Not, Or, Xor

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from proof_toolkit import load_streams, replay, require_proved  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OPS_JSON = os.path.join(HERE, os.pardir, "mod_add_qq_ops.json")

# secp256k1 prime, exactly as src/point_add/mod.rs (SECP256K1_P) — little-endian.
P = (1 << 256) - (1 << 32) - 977


def const_bits(value, n):
    """Little-endian list of z3 Bool constants for `value` over `n` bits."""
    return [BoolVal((value >> i) & 1 == 1) for i in range(n)]


def add_bits(x, y):
    """Ripple-carry add of two little-endian bit lists. Returns (sum_bits, carry_out).

    `sum_bits` has the same length as the inputs; `carry_out` is the (n+1)-th bit."""
    assert len(x) == len(y)
    carry = BoolVal(False)
    out = []
    for xi, yi in zip(x, y):
        out.append(Xor(Xor(xi, yi), carry))
        carry = Or(And(xi, yi), And(xi, carry), And(yi, carry))
    return out, carry


def sub_bits(x, y):
    """Ripple-borrow subtract `x - y` (little-endian). Returns (diff_bits, borrow_out).

    `borrow_out` is true iff `x < y` (unsigned)."""
    assert len(x) == len(y)
    borrow = BoolVal(False)
    out = []
    for xi, yi in zip(x, y):
        out.append(Xor(Xor(xi, yi), borrow))
        borrow = Or(And(Not(xi), yi), And(Not(xi), borrow), And(yi, borrow))
    return out, borrow


def ult(x, y):
    """True iff `x < y` (unsigned, little-endian bit lists)."""
    _, borrow = sub_bits(x, y)
    return borrow


def reference_mod_add(acc_in, a_in, p):
    """Independent spec for `(acc + a) mod p`, given `acc, a ∈ [0, p)`.

    Ripple-carry add to an (n+1)-bit sum `s ∈ [0, 2p)`, then subtract `p` once iff
    `s >= p`. Structurally distinct from the emitted Solinas (+c / overflow) path."""
    n = len(acc_in)
    sum_bits, carry = add_bits(acc_in, a_in)
    s = sum_bits + [carry]  # (n+1)-bit sum
    p_ext = const_bits(p, n + 1)
    s_lt_p = ult(s, p_ext)  # s < p  ⇒ no reduction
    s_minus_p, _ = sub_bits(s, p_ext)  # s - p (valid when s >= p)
    # result < p < 2^256, so its low n bits are the answer (bit n is 0).
    return [If(s_lt_p, s[i], s_minus_p[i]) for i in range(n)]


def prove_stream(stream):
    n = stream["n"]
    acc_ids, a_ids = stream["acc"], stream["a"]
    assert n == len(acc_ids) == len(a_ids) == 256

    acc_in = [Bool(f"acc{i}") for i in range(n)]
    a_in = [Bool(f"a{i}") for i in range(n)]
    inputs = {}
    for qid, v in zip(acc_ids, acc_in):
        inputs[qid] = v
    for qid, v in zip(a_ids, a_in):
        inputs[qid] = v

    st = replay(stream.ops, qubit_inputs=inputs)

    # Preconditions: both operands are field elements in [0, p).
    p_bits = const_bits(P, n)
    assumptions = [ult(acc_in, p_bits), ult(a_in, p_bits)]

    ref = reference_mod_add(acc_in, a_in, P)
    ancilla_ids = [q for q in st.qubits if q not in acc_ids and q not in a_ids]
    claims = []
    claims += [st.q(acc_ids[i]) == ref[i] for i in range(n)]        # functional
    claims += [st.q(a_ids[i]) == a_in[i] for i in range(n)]         # a preserved
    claims += [st.q(q) == BoolVal(False) for q in ancilla_ids]      # flag + ancilla clean
    claims += [st.phase == BoolVal(False)]                         # phase clean
    require_proved(claims, f"n={n}", assumptions)
    return len(st.meas), len(ancilla_ids)


def main():
    print("=" * 74)
    print(" Solinas modular reduction proved over the EMITTED mod_add_qq op-stream")
    print(" (emitter-bound complement to solinas_reduction.py / the Kani twin; F2)")
    print("=" * 74)
    if not os.path.exists(OPS_JSON):
        raise SystemExit(
            f"missing {OPS_JSON}\n  regenerate: MODADD_OPS_JSON=analysis/mod_add_qq_ops.json "
            "cargo test --release --lib modadd_dump::dump_mod_add_qq_ops -- --ignored"
        )
    streams = load_streams(OPS_JSON)
    print()
    print("  mod_add_qq, replayed through the proof_toolkit z3 model of src/sim.rs,")
    print("  R-reset outcomes free (∀), proving over all acc,a ∈ [0,p):")
    print("  acc'=(acc+a) mod p ∧ a preserved ∧ flag/ancilla clean ∧ phase 0.")
    print()
    for stream in streams:
        nmeas, nanc = prove_stream(stream)
        print(
            f"  [PROVED] n={stream['n']}: acc'=(acc+a) mod p over ALL acc,a<p; "
            f"a preserved; {nanc} ancilla (incl. flag) → |0>; "
            f"phase=0 ∀ {nmeas} measurement outcomes"
        )
    print()
    print("=" * 74)
    print(" RESULT: the Solinas reduction is PROVED correct, clean, and phase-clean")
    print(" over the emitted gates at production width — no longer only a model/twin.")
    print(" The copy↔emitter gap (F2) is now closed for the reduction too.")
    print("=" * 74)
    return 0


if __name__ == "__main__":
    sys.exit(main())
