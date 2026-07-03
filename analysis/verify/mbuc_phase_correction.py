#!/usr/bin/env python3
"""F1/F2 (referee review `paper/REVIEW.md`, ADR 0027) — a **z3 proof of the emitted
`_fast` adder's measurement-based uncompute**: the HMR + `cz_if` phase correction the
scored circuit actually runs, which the existing z3/Kani layer does NOT cover.

The scored hot path emits `mod_add_qq_fast` / `mod_sub_qq_fast` / `mod_double_inplace_fast`
(58 fast calls vs 3 plain `mod_add_qq`), all routing their addition through
`cuccaro_add_fast` (`src/point_add/arith/adder.rs`). Its backward UMA sweep clears each
carry ancilla with Gidney measurement-based uncomputation — `b.hmr(carry, m)` (X-basis
measure → random outcome `m`, qubit reset to |0>, phase kickback `carry·m`) followed by
`b.cz_if(x, y, m)` (a conditional CZ that applies phase `m·x·y`). This is **0 Toffoli**,
but its correctness rests on the two phase terms cancelling — `carry·m ⊕ x·y·m = 0` — for
**every** measurement outcome, which requires `x·y` to still equal the AND that produced
`carry` at uncompute time. The `solinas_reduction.py` z3 proof and the Kani harnesses
model only the *plain* `mod_add_qq` and treat adders as exact integer `+`, so this
phase-kickback logic has **zero symbolic coverage** — validated only by the 9024-shot
sample (findings F1/F2).

This closes that gap. It **replays the actually emitted op-stream** (dumped by the real
`B` builder into `analysis/mbuc_fast_adder_ops.json` — see `src/point_add/mbuc_dump.rs`,
so a drift between "the copy" and the emitted gates cannot pass silently, the exact F2
concern) through a z3 model of `src/sim.rs`'s per-op semantics, with the measurement
outcomes as **free, universally-quantified** booleans (not the random XOF), and proves,
for all inputs and all outcomes:

  1. FUNCTIONAL   `acc' == (a + acc) mod 2^n`             (the add is correct)
  2. CLEAN        `a` unchanged, `c_in` and every carry ancilla back to |0>
  3. PHASE-CLEAN  net global phase `== 0` for **every** measurement outcome
                  (the HMR kickback is exactly cancelled by the cz_if correction)

A **teeth** check confirms the correction is load-bearing: deleting the `cz_if` phase
corrections makes claim (3) FALSE (z3 finds an input+outcome with net phase 1).

Widths 2..256 (incl. the production 256-bit coordinate register). z3 discharges each by
returning `unsat` on the negation — a proof over all inputs and all measurement outcomes,
not a sample. Analysis-only; `#[cfg(test)]`-equivalent, never touches the scored circuit.
"""
import json
import os
import sys

from z3 import And, Bool, BoolVal, Not, Or, Solver, Xor, sat, unsat

HERE = os.path.dirname(os.path.abspath(__file__))
OPS_JSON = os.path.join(HERE, os.pardir, "mbuc_fast_adder_ops.json")


def _maj(a, b, c):
    """Boolean majority (carry-out of a full adder)."""
    return Or(And(a, b), And(a, c), And(b, c))


def replay(width, drop_cz=False):
    """Symbolically execute one emitted `cuccaro_add_fast` op-stream.

    Returns (solver-free) the symbolic end state and the input handles needed to state
    the claims: qubit values (dict id->Bool expr), the global `phase` Bool, the input
    a/acc/c_in handles, the fresh measurement-outcome vars, and the ancilla ids.

    `drop_cz=True` omits the `cz_if` phase corrections (the teeth variant)."""
    n = width["n"]
    a_ids, acc_ids, c_in_id = width["a"], width["acc"], width["c_in"]
    ops = width["ops"]

    # Qubit initial state: a[], acc[] free inputs; c_in and every ancilla start at |0>.
    a_in = [Bool(f"a{i}") for i in range(n)]
    acc_in = [Bool(f"acc{i}") for i in range(n)]
    qval = {}
    for i, qid in enumerate(a_ids):
        qval[qid] = a_in[i]
    for i, qid in enumerate(acc_ids):
        qval[qid] = acc_in[i]
    qval[c_in_id] = BoolVal(False)  # documented precondition: c_in is a |0> ancilla

    def q(qid):
        return qval.get(qid, BoolVal(False))  # unseen ids are |0> ancilla

    bits = {}

    def b(bid):
        return bits.get(bid, BoolVal(False))

    phase = BoolVal(False)
    meas = []  # fresh measurement-outcome vars (universally quantified)
    mcount = 0

    for op in ops:
        kind, qc2, qc1, qt, ct, cc = op
        cond = BoolVal(True) if cc < 0 else b(cc)
        if kind == "CX":
            qval[qt] = Xor(q(qt), And(cond, q(qc1)))
        elif kind == "CCX":
            qval[qt] = Xor(q(qt), And(cond, q(qc1), q(qc2)))
        elif kind == "X":
            qval[qt] = Xor(q(qt), cond)
        elif kind == "CZ":
            if not drop_cz:
                phase = Xor(phase, And(cond, q(qt), q(qc1)))
        elif kind == "Z":
            if not drop_cz:
                phase = Xor(phase, And(cond, q(qt)))
        elif kind == "CCZ":
            phase = Xor(phase, And(cond, q(qt), q(qc1), q(qc2)))
        elif kind == "NEG":
            phase = Xor(phase, cond)
        elif kind == "HMR":
            m = Bool(f"m{mcount}")
            mcount += 1
            meas.append(m)
            # bit[ct] := m (cond true here); phase ^= q_target & m; q_target := 0.
            bits[ct] = m if cc < 0 else Xor(And(Not(cond), b(ct)), And(cond, m))
            phase = Xor(phase, And(q(qt), m, cond))
            qval[qt] = And(q(qt), Not(cond))
        elif kind == "R":
            m = Bool(f"r{mcount}")
            mcount += 1
            meas.append(m)
            phase = Xor(phase, And(q(qt), m, cond))
            qval[qt] = And(q(qt), Not(cond))
        else:
            raise AssertionError(f"unmodeled op kind {kind}")

    ancilla_ids = [qid for qid in qval if qid not in a_ids and qid != c_in_id and qid not in acc_ids]
    return {
        "n": n,
        "a_in": a_in,
        "acc_in": acc_in,
        "a_out": [q(qid) for qid in a_ids],
        "acc_out": [q(qid) for qid in acc_ids],
        "c_in_out": q(c_in_id),
        "ancilla_out": [q(qid) for qid in ancilla_ids],
        "phase": phase,
        "meas": meas,
    }


def expected_sum_bits(a_in, acc_in):
    """Ripple-carry reference: (a + acc) mod 2^n, carry-in 0, bit by bit."""
    carry = BoolVal(False)
    out = []
    for i in range(len(a_in)):
        out.append(Xor(Xor(a_in[i], acc_in[i]), carry))
        carry = _maj(a_in[i], acc_in[i], carry)
    return out


def prove_width(width):
    n = width["n"]
    st = replay(width)

    # (1) FUNCTIONAL + (2) CLEAN + (3) PHASE-CLEAN — negate the conjunction, expect unsat.
    ref = expected_sum_bits(st["a_in"], st["acc_in"])
    claims = []
    claims += [st["acc_out"][i] == ref[i] for i in range(n)]              # functional
    claims += [st["a_out"][i] == st["a_in"][i] for i in range(n)]          # a preserved
    claims += [st["c_in_out"] == BoolVal(False)]                          # c_in clean
    claims += [av == BoolVal(False) for av in st["ancilla_out"]]           # carries clean
    claims += [st["phase"] == BoolVal(False)]                            # phase clean

    s = Solver()
    s.add(Not(And(*claims)))
    res = s.check()
    if res != unsat:
        raise SystemExit(f"[FAIL] width {n}: claim not proved (z3 returned {res})")

    # Teeth: without the cz_if corrections, phase-clean must FAIL (sat: exists a
    # counterexample input+outcome giving net phase 1).
    st_no = replay(width, drop_cz=True)
    t = Solver()
    t.add(st_no["phase"] == BoolVal(True))
    teeth = t.check()
    if teeth != sat:
        raise SystemExit(f"[FAIL] width {n}: teeth check did not fire (z3 {teeth})")

    return len(st["meas"])


def main():
    print("=" * 74)
    print(" F1/F2 (ADR 0027): z3 proof of the emitted _fast adder's measurement-based")
    print(" uncompute (HMR + cz_if phase correction) — over the REAL emitted op-stream")
    print("=" * 74)
    if not os.path.exists(OPS_JSON):
        raise SystemExit(
            f"missing {OPS_JSON}\n  regenerate: MBUC_OPS_JSON=analysis/mbuc_fast_adder_ops.json "
            "cargo test --release --lib mbuc_dump::dump_fast_adder_ops -- --ignored"
        )
    data = json.load(open(OPS_JSON))
    print()
    print("  cuccaro_add_fast, replayed through a z3 model of src/sim.rs, measurement")
    print("  outcomes free (∀), proving: add correct ∧ registers clean ∧ net phase 0.")
    print()
    for width in data["widths"]:
        nmeas = prove_width(width)
        print(
            f"  [PROVED] n={width['n']:>3}: acc'=(a+acc) mod 2^{width['n']}, "
            f"a/c_in/carries clean, phase=0 ∀ inputs ∧ ∀ {nmeas} outcomes; "
            f"teeth: drop cz_if ⇒ phase≠0 [sat]"
        )
    print()
    print("=" * 74)
    print(f" RESULT: the emitted _fast adder's HMR + cz_if phase correction is PROVED")
    print(f" phase-clean and functionally correct over all inputs and all measurement")
    print(f" outcomes, at every width incl. the production 256 — the F1/F2 gap closed.")
    print("=" * 74)
    return 0


if __name__ == "__main__":
    sys.exit(main())
