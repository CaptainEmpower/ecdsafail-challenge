"""Symbolic replay of an emitted kickmix op-stream through a z3 model of
`src/sim.rs`'s per-op semantics.

This is the reusable core of the verification methodology recorded in
`analysis/adr/0028-reusable-proof-toolkit.md`: the generalized z3 op-stream
replayer, extracted from the ADR 0027 proof (`mbuc_phase_correction.py`).

**Why this is the portable artifact.** The scored circuit's *gates* are
score-specialized and must stay byte-identical (ADR 0001/0028), so they are not
carved into a library. The transferable science is the *how-to-verify*: take the
op-stream the real `B` builder emits — dumped to JSON by a `#[cfg(test)]` harness
whose drift guard keeps the artifact byte-identical to a fresh emit (the ADR 0027
`mbuc_dump.rs` pattern) — and prove a claimed property over **all inputs and all
measurement outcomes** by replaying it through a faithful symbolic model of the
simulator that scores it. The proof then verifies *what you run*, not a
re-implementation (the F2 concern, generalized).

`SymSim` mirrors `Simulator::apply_iter` (`src/sim.rs`) op-for-op. Each of the 64
parallel shots in the real u64-masked simulator is independent, so one symbolic
shot — z3 `Bool` per qubit / classical bit, one `Bool` global phase — models every
shot. The per-shot condition mask `cond` becomes a `Bool`; `& cond` becomes
`And(..., cond)`; each `HMR`/`R` measurement outcome (the random XOF byte in the
real sim) becomes a **fresh free `Bool`**, so a claim proved here holds for *every*
outcome, not the sampled ones.

Op-stream JSON format (as emitted by `mbuc_dump.rs::ops_to_json`): each op is the
6-tuple ``[kind, q_control2, q_control1, q_target, c_target, c_condition]`` where
`kind` is the `OperationType` name (`"CX"`, `"CCX"`, `"HMR"`, …) and qubit/bit ids
are non-negative ints, `-1` for absent (`NO_QUBIT`/`NO_BIT`). A stream bundle is
``{"widths": [ {..metadata.., "ops": [ ...op tuples... ]}, ... ]}``.
"""
from __future__ import annotations

import json
from dataclasses import dataclass, field

from z3 import And, Bool, BoolRef, BoolVal, Not, Or, Solver, Xor, sat, unsat

# Op-tuple field positions (mirrors `mbuc_dump.rs::ops_to_json`).
_KIND, _QC2, _QC1, _QT, _CT, _CC = range(6)

# Op kinds whose only effect is on the global phase. `drop_phase_kinds` may name
# any of these to suppress its phase contribution — the general form of the ADR
# 0027 teeth check (delete the `cz_if` fixups ⇒ the phase claim must break).
PHASE_OP_KINDS = frozenset({"CZ", "Z", "CCZ", "NEG"})


@dataclass
class SymState:
    """The symbolic end state of a replayed op-stream.

    `qubits`/`bits` map id → final z3 `Bool` expression; ids never written keep
    their initial value (|0> / False unless supplied as an input). `phase` is the
    net global phase `Bool`. `meas` collects the fresh measurement-outcome vars
    introduced by `HMR`/`R` (the universally-quantified outcomes)."""

    qubits: dict[int, BoolRef]
    bits: dict[int, BoolRef]
    phase: BoolRef
    meas: list[BoolRef]

    def q(self, qid: int) -> BoolRef:
        """Final value of qubit `qid` (|0> if never touched)."""
        return self.qubits.get(qid, BoolVal(False))

    def bit(self, bid: int) -> BoolRef:
        """Final value of classical bit `bid` (False if never touched)."""
        return self.bits.get(bid, BoolVal(False))


class SymSim:
    """A symbolic executor of the `src/sim.rs` op semantics over z3 `Bool`s.

    Qubits/bits are lazily |0>/False until written; pass `qubit_inputs` /
    `bit_inputs` to seed the free-input registers. Construct, `apply(op)` a stream
    (or `apply_all(ops)`), then read `.state()`."""

    def __init__(
        self,
        qubit_inputs: dict[int, BoolRef] | None = None,
        bit_inputs: dict[int, BoolRef] | None = None,
        *,
        drop_phase_kinds: frozenset[str] = frozenset(),
        meas_prefix: str = "m",
    ):
        if not drop_phase_kinds <= PHASE_OP_KINDS:
            raise ValueError(
                f"drop_phase_kinds must be a subset of {sorted(PHASE_OP_KINDS)}; "
                f"got {sorted(drop_phase_kinds)}"
            )
        self._qval: dict[int, BoolRef] = dict(qubit_inputs or {})
        self._bits: dict[int, BoolRef] = dict(bit_inputs or {})
        self._drop = drop_phase_kinds
        self._meas_prefix = meas_prefix
        self.phase: BoolRef = BoolVal(False)
        self.meas: list[BoolRef] = []
        # Mirrors `Simulator::apply_iter`'s `current_base_condition` (u64::MAX ⇒ all
        # shots ⇒ True) and its push/pop stack.
        self._base_cond: BoolRef = BoolVal(True)
        self._cond_stack: list[BoolRef] = []

    # -- state access -------------------------------------------------------
    def q(self, qid: int) -> BoolRef:
        if qid < 0:
            return BoolVal(False)
        return self._qval.get(qid, BoolVal(False))

    def bit(self, bid: int) -> BoolRef:
        if bid < 0:
            return BoolVal(False)
        return self._bits.get(bid, BoolVal(False))

    def state(self) -> SymState:
        return SymState(
            qubits=dict(self._qval),
            bits=dict(self._bits),
            phase=self.phase,
            meas=list(self.meas),
        )

    # -- execution ----------------------------------------------------------
    def _fresh_meas(self) -> BoolRef:
        m = Bool(f"{self._meas_prefix}{len(self.meas)}")
        self.meas.append(m)
        return m

    def _phase_xor(self, kind: str, term: BoolRef) -> None:
        """Add `term` to the global phase unless this phase-op kind is dropped."""
        if kind in self._drop:
            return
        self.phase = Xor(self.phase, term)

    def apply(self, op) -> None:
        kind = op[_KIND]
        qc2, qc1, qt = op[_QC2], op[_QC1], op[_QT]
        ct, cc = op[_CT], op[_CC]

        # cond = current_base_condition & (bit(c_condition) if present)
        cond = self._base_cond if cc < 0 else And(self._base_cond, self.bit(cc))

        if kind == "CCX":
            self._qval[qt] = Xor(self.q(qt), And(cond, self.q(qc1), self.q(qc2)))
        elif kind == "CX":
            self._qval[qt] = Xor(self.q(qt), And(cond, self.q(qc1)))
        elif kind == "X":
            self._qval[qt] = Xor(self.q(qt), cond)
        elif kind == "SWAP":
            # Faithful to sim.rs's conditional 3-XOR swap of (q_control1, q_target).
            # Canonical op name is uppercase "SWAP" (circuit.rs::from_name / mbuc_dump.rs).
            q_c1 = Xor(self.q(qc1), self.q(qt))          # q_c1 ^= q_t
            q_t = Xor(self.q(qt), And(cond, q_c1))        # q_t  ^= cond & q_c1
            q_c1 = Xor(q_c1, q_t)                         # q_c1 ^= q_t
            self._qval[qc1] = q_c1
            self._qval[qt] = q_t
        elif kind == "CCZ":
            self._phase_xor(kind, And(cond, self.q(qt), self.q(qc1), self.q(qc2)))
        elif kind == "CZ":
            self._phase_xor(kind, And(cond, self.q(qt), self.q(qc1)))
        elif kind == "Z":
            self._phase_xor(kind, And(cond, self.q(qt)))
        elif kind == "NEG":
            self._phase_xor(kind, cond)
        elif kind == "HMR":
            # X-basis measure → fresh random outcome m; bit(ct):=m under cond;
            # phase ^= q_target & m & cond; q_target reset to |0> under cond.
            m = self._fresh_meas()
            self._bits[ct] = Xor(And(self.bit(ct), Not(cond)), And(m, cond))
            self.phase = Xor(self.phase, And(self.q(qt), m, cond))
            self._qval[qt] = And(self.q(qt), Not(cond))
        elif kind == "R":
            # Like HMR but no classical-bit record.
            m = self._fresh_meas()
            self.phase = Xor(self.phase, And(self.q(qt), m, cond))
            self._qval[qt] = And(self.q(qt), Not(cond))
        elif kind == "BIT_INVERT":
            self._bits[ct] = Xor(self.bit(ct), cond)
        elif kind == "BIT_STORE0":
            self._bits[ct] = And(self.bit(ct), Not(cond))
        elif kind == "BIT_STORE1":
            self._bits[ct] = Or(self.bit(ct), cond)
        elif kind == "PUSH_CONDITION":
            self._cond_stack.append(self._base_cond)
            self._base_cond = And(self._base_cond, self.bit(cc))
        elif kind == "POP_CONDITION":
            if self._cond_stack:
                self._base_cond = self._cond_stack.pop()
        elif kind in ("APPEND_TO_REGISTER", "REGISTER", "DEBUG_PRINT"):
            pass  # bookkeeping ops, no simulator effect
        else:
            raise ValueError(f"unmodeled op kind {kind!r}")

    def apply_all(self, ops) -> "SymSim":
        for op in ops:
            self.apply(op)
        return self


def replay(
    ops,
    *,
    qubit_inputs: dict[int, BoolRef] | None = None,
    bit_inputs: dict[int, BoolRef] | None = None,
    drop_phase_kinds: frozenset[str] = frozenset(),
    meas_prefix: str = "m",
) -> SymState:
    """Replay `ops` from a fresh symbolic state and return the end `SymState`.

    `qubit_inputs`/`bit_inputs` seed the free-input registers (id → z3 `Bool`);
    everything else starts |0>/False. `drop_phase_kinds` suppresses the phase
    contribution of the named phase ops (the teeth lever). `meas_prefix` names the
    fresh `HMR`/`R` outcome vars."""
    sim = SymSim(
        qubit_inputs=qubit_inputs,
        bit_inputs=bit_inputs,
        drop_phase_kinds=drop_phase_kinds,
        meas_prefix=meas_prefix,
    )
    return sim.apply_all(ops).state()


# -- op-stream loading ------------------------------------------------------
@dataclass
class OpStream:
    """One emitted op-stream plus its metadata, loaded from the JSON bundle.

    `meta` carries every non-`ops` key from the JSON object (e.g. the mbuc dump's
    `n`, `a`, `acc`, `c_in`); `ops` is the list of 6-tuple op records."""

    ops: list
    meta: dict = field(default_factory=dict)

    def __getitem__(self, key):
        return self.meta[key]

    def get(self, key, default=None):
        return self.meta.get(key, default)


def load_streams(path: str) -> list[OpStream]:
    """Load a `{"widths": [...]}` op-stream bundle (the `mbuc_dump.rs` format).

    Ignores top-level metadata keys (e.g. `_comment`) and returns one `OpStream`
    per entry in `widths`, splitting each entry's `ops` from its metadata."""
    with open(path) as fh:
        data = json.load(fh)
    out = []
    for w in data["widths"]:
        meta = {k: v for k, v in w.items() if k != "ops"}
        out.append(OpStream(ops=w["ops"], meta=meta))
    return out


# -- z3 claim helpers -------------------------------------------------------
def prove(claims: list[BoolRef], assumptions: list[BoolRef] = ()):
    """Try to prove the conjunction of `claims` over all free vars.

    `assumptions` are preconditions added to the solver (e.g. `a < p`, `acc < p`);
    the claim is then proved *under* them. Returns z3's result on the negation of
    the claims: `unsat` means proved (no input / measurement outcome satisfying the
    assumptions violates any claim); `sat` means a counterexample exists."""
    s = Solver()
    for a in assumptions:
        s.add(a)
    s.add(Not(And(*claims)))
    return s.check()


def find(expr: BoolRef):
    """Search for a model satisfying `expr` (the teeth direction).

    Returns `sat` if some input/outcome makes `expr` true (e.g. net phase 1 after
    dropping a load-bearing correction), else `unsat`."""
    s = Solver()
    s.add(expr)
    return s.check()


def require_proved(claims: list[BoolRef], label: str, assumptions: list[BoolRef] = ()) -> None:
    """Assert `claims` hold for all inputs/outcomes (under `assumptions`), else raise."""
    res = prove(claims, assumptions)
    if res != unsat:
        raise SystemExit(f"[FAIL] {label}: claim not proved (z3 returned {res})")


def require_teeth(expr: BoolRef, label: str) -> None:
    """Assert `expr` is satisfiable (a load-bearing correction really bites)."""
    res = find(expr)
    if res != sat:
        raise SystemExit(f"[FAIL] {label}: teeth check did not fire (z3 returned {res})")


# Reference semantics small callers reuse when stating claims.
def ripple_carry_sum(a_bits: list[BoolRef], b_bits: list[BoolRef]) -> list[BoolRef]:
    """`(a + b) mod 2^n` as a symbolic little-endian bit list, carry-in 0."""
    if len(a_bits) != len(b_bits):
        raise ValueError("ripple_carry_sum: operand widths differ")
    carry = BoolVal(False)
    out = []
    for ai, bi in zip(a_bits, b_bits):
        out.append(Xor(Xor(ai, bi), carry))
        carry = Or(And(ai, bi), And(ai, carry), And(bi, carry))
    return out


__all__ = [
    "SymSim",
    "SymState",
    "OpStream",
    "replay",
    "load_streams",
    "prove",
    "find",
    "require_proved",
    "require_teeth",
    "ripple_carry_sum",
    "PHASE_OP_KINDS",
]
