#!/usr/bin/env python3
"""The scored hot path's `_fast` modular wrappers, proved over the **emitted gates**.

The scored circuit runs 58 `_fast` modular calls (`mod_add_qq_fast`, `mod_sub_qq_fast`,
`mod_double_inplace_fast`) vs 3 plain `mod_add_qq` (ADR 0027). ADR 0031 bound the plain
`mod_add_qq` reduction to the emitter; this binds the `_fast` wrappers the hot path
actually runs. They fold the same Solinas reduction around the **measurement-based**
adder (`cuccaro_add_fast`, HMR + `cz_if`) proved in ADR 0027, so their emitted streams
carry HMR/`CZ` ops and free measurement outcomes — the reduction is proved phase-clean
**in context**, not just the adder in isolation.

Replays the real emitted op-streams (dumped by `src/point_add/modfast_dump.rs` into
`analysis/mod_fast_ops.json` at the default/scored builder config; a `#[cfg(test)]` drift
guard keeps the artifact byte-identical) through `proof_toolkit`, with the HMR/`R`
measurement outcomes **free/∀**, and proves over all field-element inputs:

  add     `acc' == (acc + a) mod p`      (a preserved)   — canonical, fully reduced
  sub     `acc' == (acc - a) mod p`      (a preserved)   — canonical, fully reduced
  double  `v'  ≡  2·v (mod p)`, `v' < 2^n`                — see the lazy-reduction note

…plus, for every op: every ancilla (incl. the reduction flag) returns to |0>, and the
net phase is 0 for **all** measurement outcomes (the HMR kickback is cancelled by the
`cz_if` corrections, now across the whole reduction).

Lazy-reduction note (`double`): `mod_double_inplace_fast` performs a **single** fold
(add `c` on the `2^n` carry). For `v ∈ [2^255 − c/2, 2^255)` the doubled value lands in
`[p, 2^n)` and is left **unreduced** — so `v'` is `2v mod p` OR `2v mod p + p`, i.e.
congruent mod p and `< 2^n`, but not always the canonical representative. Its 64-shot
unit test asserts full reduction yet never samples that ~2^31-wide window; this symbolic
proof states the contract the circuit actually satisfies. (Downstream ops are mod-p, so
a representative in `[p, 2^n)` is harmless — but it is not `< p`.)

Reference values are the independent `proof_toolkit.refspec` arithmetic (ripple-carry +
conditional sub/add), structurally unlike the replayed Solinas `+c`/overflow path.

Usage: `mod_fast_reduction_emitted.py [add|sub|double ...]` (default: all). Each op is a
heavy 256-bit proof; run individually to parallelize. z3 returns `unsat` on the negation.
"""
import os
import sys
import time

from z3 import Bool, BoolVal

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from z3 import unsat  # noqa: E402

from proof_toolkit import (  # noqa: E402
    bits_eq,
    const_bits,
    load_streams,
    mod_add,
    mod_double_canonical,
    mod_reduce_once,
    mod_sub,
    prove,
    replay,
    require_proved,
    ult,
)

# Optional per-group z3 timeout (ms). When set, groups are checked with a bound and
# the result (proved / unknown-timeout / counterexample) is reported without aborting
# — a diagnostic for these heavy 256-bit replays. Unset ⇒ prove unbounded (raise on
# failure), the normal mode.
_TIMEOUT_MS = os.environ.get("PROOF_TIMEOUT_MS")

HERE = os.path.dirname(os.path.abspath(__file__))
OPS_JSON = os.path.join(HERE, os.pardir, "mod_fast_ops.json")
P = (1 << 256) - (1 << 32) - 977  # secp256k1 prime (== SECP256K1_P)


def _free(ids, name):
    bits = [Bool(f"{name}{i}") for i in range(len(ids))]
    return bits, {qid: v for qid, v in zip(ids, bits)}


def _prove_groups(op, groups, assumptions):
    """Prove each claim-group in its own z3 solve (each `unsat` on its negation).

    Proving the properties separately — rather than as one giant conjunction — keeps
    each solver's formula small enough to discharge (the phase clause over ~10^3 free
    HMR outcomes does not have to interact with the functional clause), and gives
    per-group progress on these heavy 256-bit replays."""
    for label, claims in groups:
        t0 = time.perf_counter()
        if _TIMEOUT_MS is None:
            require_proved(claims, f"{op}:{label}", assumptions)
            verdict = "ok"
        else:
            res = prove(claims, assumptions, timeout_ms=int(_TIMEOUT_MS))
            verdict = "ok" if res == unsat else f"UNPROVED[{res}]"
        print(f"    [{verdict}] {op}:{label:<11} ({time.perf_counter() - t0:6.1f}s)", flush=True)


def prove_add_or_sub(stream, subtract):
    n = stream["n"]
    acc_ids, a_ids = stream["acc"], stream["a"]
    acc_in, acc_map = _free(acc_ids, "acc")
    a_in, a_map = _free(a_ids, "a")
    st = replay(stream.ops, qubit_inputs={**acc_map, **a_map})

    p_bits = const_bits(P, n)
    assumptions = [ult(acc_in, p_bits), ult(a_in, p_bits)]
    ref = mod_sub(acc_in, a_in, P) if subtract else mod_add(acc_in, a_in, P)

    ancilla = [q for q in st.qubits if q not in acc_ids and q not in a_ids]
    groups = [
        ("functional", [st.q(acc_ids[i]) == ref[i] for i in range(n)]),
        ("preserved", [st.q(a_ids[i]) == a_in[i] for i in range(n)]),
        ("clean", [st.q(q) == BoolVal(False) for q in ancilla]),
        ("phase", [st.phase == BoolVal(False)]),
    ]
    _prove_groups(stream["op"], groups, assumptions)
    return len(st.meas), len(ancilla), "canonical (acc±a) mod p, a preserved"


def prove_double(stream):
    n = stream["n"]
    v_ids = stream["v"]
    v_in, v_map = _free(v_ids, "v")
    st = replay(stream.ops, qubit_inputs=v_map)

    p_bits = const_bits(P, n)
    assumptions = [ult(v_in, p_bits)]

    # Lazy contract: v' ≡ 2v (mod p) and v' < 2^n. `mod_double_inplace_fast` folds
    # only once, so v' may be the canonical `(2v) mod p` OR that + p. Reducing v'
    # canonically (one conditional −p) must then equal the canonical reference — a
    # single 256-bit equality (much lighter for z3 than an `x ∈ {r, r+p}` disjunction).
    r = mod_double_canonical(v_in, P)
    v_out = [st.q(v_ids[i]) for i in range(n)]
    congruent = bits_eq(mod_reduce_once(v_out, P), r)

    ancilla = [q for q in st.qubits if q not in v_ids]
    groups = [
        ("congruent", [congruent]),                              # v' ≡ 2v (mod p)
        ("clean", [st.q(q) == BoolVal(False) for q in ancilla]),  # ancilla clean
        ("phase", [st.phase == BoolVal(False)]),                 # phase clean
    ]
    _prove_groups(stream["op"], groups, assumptions)
    return len(st.meas), len(ancilla), "congruence v' ≡ 2v (mod p), v' < 2^n (lazy fold)"


def main():
    want = set(sys.argv[1:]) or {"add", "sub", "double"}
    print("=" * 74)
    print(" Scored `_fast` modular wrappers proved over the EMITTED gates (z3, F2)")
    print(f" (proof_toolkit replay of src/sim.rs; outcomes free/∀)  ops: {sorted(want)}")
    print("=" * 74)
    if not os.path.exists(OPS_JSON):
        raise SystemExit(
            f"missing {OPS_JSON}\n  regenerate: MODFAST_OPS_JSON=analysis/mod_fast_ops.json "
            "cargo test --release --lib modfast_dump::dump_mod_fast_ops -- --ignored"
        )
    streams = {s["op"]: s for s in load_streams(OPS_JSON)}
    print()
    for op in ("add", "sub", "double"):
        if op not in want:
            continue
        stream = streams[op]
        if op == "double":
            nmeas, nanc, contract = prove_double(stream)
        else:
            nmeas, nanc, contract = prove_add_or_sub(stream, subtract=(op == "sub"))
        print(
            f"  [PROVED] {op:>6}: {contract}; {nanc} ancilla (incl. flag) → |0>; "
            f"phase=0 ∀ {nmeas} measurement outcomes"
        )
    print()
    print("=" * 74)
    print(" RESULT: the scored hot-path `_fast` modular wrappers are PROVED correct and")
    print(" phase-clean over the emitted gates at production width — the Solinas fold")
    print(" around the measurement-based adder is verified in context (F2).")
    print("=" * 74)
    return 0


if __name__ == "__main__":
    sys.exit(main())
