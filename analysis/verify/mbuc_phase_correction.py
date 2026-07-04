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

The symbolic sim.rs replayer this proof drives is the reusable `proof_toolkit` package
(ADR 0028/0029, extracted from this proof's original private replay); this script is now
its first consumer — it states the adder-specific claims, the toolkit owns the op
semantics.
"""
import os
import sys

from z3 import Bool, BoolVal

# `proof_toolkit` lives alongside this script in analysis/verify/; make it importable
# whether the script is run from analysis/ (the justfile CWD) or from verify/.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from proof_toolkit import (  # noqa: E402
    load_streams,
    replay,
    require_proved,
    require_teeth,
    ripple_carry_sum,
)

HERE = os.path.dirname(os.path.abspath(__file__))
OPS_JSON = os.path.join(HERE, os.pardir, "mbuc_fast_adder_ops.json")

# Dropping the `cz_if` phase corrections (emitted as CZ; Z included defensively) is the
# teeth lever: without them the HMR kickback survives and the phase claim must break.
_CZ_IF_KINDS = frozenset({"CZ", "Z"})


def _adder_inputs(stream):
    """Free-input map for one emitted `cuccaro_add_fast` stream.

    `a[]` and `acc[]` are free boolean inputs; `c_in` and every carry ancilla start
    at |0> (omitted ⇒ the toolkit defaults them to False — the documented
    precondition that `c_in` is a fresh |0> ancilla)."""
    n = stream["n"]
    a_in = [Bool(f"a{i}") for i in range(n)]
    acc_in = [Bool(f"acc{i}") for i in range(n)]
    inputs = {}
    for qid, val in zip(stream["a"], a_in):
        inputs[qid] = val
    for qid, val in zip(stream["acc"], acc_in):
        inputs[qid] = val
    return a_in, acc_in, inputs


def prove_width(stream):
    n = stream["n"]
    a_ids, acc_ids, c_in_id = stream["a"], stream["acc"], stream["c_in"]
    a_in, acc_in, inputs = _adder_inputs(stream)

    st = replay(stream.ops, qubit_inputs=inputs)

    # (1) FUNCTIONAL + (2) CLEAN + (3) PHASE-CLEAN — proved together (∀ inputs, ∀ outcomes).
    ref = ripple_carry_sum(a_in, acc_in)
    ancilla_ids = [
        qid for qid in st.qubits if qid not in a_ids and qid not in acc_ids and qid != c_in_id
    ]
    claims = []
    claims += [st.q(acc_ids[i]) == ref[i] for i in range(n)]           # functional
    claims += [st.q(a_ids[i]) == a_in[i] for i in range(n)]            # a preserved
    claims += [st.q(c_in_id) == BoolVal(False)]                       # c_in clean
    claims += [st.q(qid) == BoolVal(False) for qid in ancilla_ids]    # carries clean
    claims += [st.phase == BoolVal(False)]                           # phase clean
    require_proved(claims, f"width {n}")

    # Teeth: without the cz_if corrections, phase-clean must FAIL (∃ input+outcome
    # giving net phase 1).
    st_no = replay(stream.ops, qubit_inputs=inputs, drop_phase_kinds=_CZ_IF_KINDS)
    require_teeth(st_no.phase == BoolVal(True), f"width {n} teeth")

    return len(st.meas)


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
    streams = load_streams(OPS_JSON)
    print()
    print("  cuccaro_add_fast, replayed through the proof_toolkit z3 model of src/sim.rs,")
    print("  measurement outcomes free (∀), proving: add correct ∧ registers clean ∧ phase 0.")
    print()
    for stream in streams:
        nmeas = prove_width(stream)
        print(
            f"  [PROVED] n={stream['n']:>3}: acc'=(a+acc) mod 2^{stream['n']}, "
            f"a/c_in/carries clean, phase=0 ∀ inputs ∧ ∀ {nmeas} outcomes; "
            f"teeth: drop cz_if ⇒ phase≠0 [sat]"
        )
    print()
    print("=" * 74)
    print(" RESULT: the emitted _fast adder's HMR + cz_if phase correction is PROVED")
    print(" phase-clean and functionally correct over all inputs and all measurement")
    print(" outcomes, at every width incl. the production 256 — the F1/F2 gap closed.")
    print("=" * 74)
    return 0


if __name__ == "__main__":
    sys.exit(main())
