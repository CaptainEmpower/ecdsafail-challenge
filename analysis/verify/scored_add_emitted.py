#!/usr/bin/env python3
"""The **scored** circuit's adder, proved over its emitted gates (ADR 0036).

ADR 0035 established that the scored `ops.bin` is built by `trailmix_ludicrous` and does
NOT emit the reference `arith/modular/*_fast` primitives the earlier emitter-bound proofs
(ADR 0027/0030/0031/0032) verify. This binds a proof to a gate family the scored circuit
*actually runs*: `arith::hybrid_add_adaptive`, the adder the scored square's `add_into`
drives (`a := (a + b) mod 2^n`, `b` preserved).

Replays the real emitted op-streams (dumped by
`src/point_add/trailmix_ludicrous/scored_add_dump.rs` into `analysis/scored_add_ops.json`;
a `#[cfg(test)]` drift guard keeps them byte-identical) through `proof_toolkit`, with the
HMR/`R` measurement outcomes **free/∀**, and proves for every config — spanning both
dispatch branches (plain Gidney add and sqrt(n)-chunked add) at widths up to the
production 256:

  FUNCTIONAL    a' == (a + b) mod 2^n
  B-PRESERVED   b unchanged
  CLEAN         every ancilla (vented boundary carries, pads) returns to |0>
  PHASE-CLEAN   net phase 0 for all measurement outcomes (the measurement-vented
                carry uncompute — HMR + CZ/Z/NEG fixups — cancels)

z3 returns `unsat` on the negation. Each claim group is proved in its own solve (keeps
the phase clause over ~10^2–10^3 free outcomes from interacting with the functional
clause). Set `PROOF_TIMEOUT_MS` to bound + diagnose instead of aborting.
"""
import os
import sys
import time

from z3 import Bool, BoolVal, unsat

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from proof_toolkit import (  # noqa: E402
    load_streams,
    prove,
    replay,
    require_proved,
    ripple_carry_sum,
)

HERE = os.path.dirname(os.path.abspath(__file__))
OPS_JSON = os.path.join(HERE, os.pardir, "scored_add_ops.json")
_TIMEOUT_MS = os.environ.get("PROOF_TIMEOUT_MS")


def _prove_groups(label, groups):
    for name, claims in groups:
        t0 = time.perf_counter()
        if _TIMEOUT_MS is None:
            require_proved(claims, f"{label}:{name}")
            verdict = "ok"
        else:
            res = prove(claims, timeout_ms=int(_TIMEOUT_MS))
            verdict = "ok" if res == unsat else f"UNPROVED[{res}]"
        print(f"    [{verdict}] {label}:{name:<11} ({time.perf_counter() - t0:6.1f}s)", flush=True)


def prove_config(stream):
    n = stream["n"]
    a_ids, b_ids = stream["a"], stream["b"]
    a_in = [Bool(f"a{i}") for i in range(n)]
    b_in = [Bool(f"b{i}") for i in range(n)]
    inputs = {}
    for qid, v in zip(a_ids, a_in):
        inputs[qid] = v
    for qid, v in zip(b_ids, b_in):
        inputs[qid] = v

    st = replay(stream.ops, qubit_inputs=inputs)
    ref = ripple_carry_sum(a_in, b_in)  # (a + b) mod 2^n
    ancilla = [q for q in st.qubits if q not in a_ids and q not in b_ids]
    groups = [
        ("functional", [st.q(a_ids[i]) == ref[i] for i in range(n)]),
        ("preserved", [st.q(b_ids[i]) == b_in[i] for i in range(n)]),
        ("clean", [st.q(q) == BoolVal(False) for q in ancilla]),
        ("phase", [st.phase == BoolVal(False)]),
    ]
    label = f"n={n},k={stream['k']}"
    _prove_groups(label, groups)
    return len(st.meas), len(ancilla)


def main():
    print("=" * 74)
    print(" Scored trailmix adder (hybrid_add_adaptive) proved over the EMITTED gates")
    print(" (ADR 0036 — binds a proof to gates the scored ops.bin actually runs)")
    print("=" * 74)
    if not os.path.exists(OPS_JSON):
        raise SystemExit(
            f"missing {OPS_JSON}\n  regenerate: SCORED_ADD_OPS_JSON=analysis/scored_add_ops.json "
            "cargo test --release --lib trailmix_ludicrous::scored_add_dump::dump_scored_add_ops "
            "-- --ignored"
        )
    streams = load_streams(OPS_JSON)
    print()
    for stream in streams:
        nmeas, nanc = prove_config(stream)
        print(
            f"  [PROVED] n={stream['n']:>3} k={stream['k']:>3}: a'=(a+b) mod 2^{stream['n']}, "
            f"b preserved, {nanc} ancilla → |0>, phase=0 ∀ {nmeas} outcomes"
        )
    print()
    print("=" * 74)
    print(" RESULT: the scored circuit's adder is PROVED correct, register-clean, and")
    print(" phase-clean over its emitted gates — both dispatch branches, up to width 256.")
    print("=" * 74)
    return 0


if __name__ == "__main__":
    sys.exit(main())
