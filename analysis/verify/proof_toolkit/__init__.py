"""`proof_toolkit` — the repo's verification methodology as a reusable library.

The transferable asset of this repo is not the hand-tuned scored gates (which stay
byte-identical, ADR 0001/0028) but the *how-to-verify* methodology: prove a claim
about the circuit you actually emit by replaying its op-stream through a faithful
symbolic model of the simulator that scores it, over all inputs and all measurement
outcomes. ADR 0028 scoped this; ADR 0029 promotes its first module — the generalized
z3 op-stream replayer, extracted from the ADR 0027 proof — into this package.

Public API (see `symsim` for details):

- `SymSim`, `replay`  — symbolic execution of an emitted op-stream (a z3 model of
  `src/sim.rs` per-op semantics; measurement outcomes are free/∀).
- `SymState`          — the symbolic end state (qubits, bits, phase, meas vars).
- `OpStream`, `load_streams` — load the `{"widths": [...]}` dump format
  (`src/point_add/mbuc_dump.rs`), splitting ops from per-stream metadata.
- `prove`, `find`, `require_proved`, `require_teeth` — z3 claim/counterexample
  helpers (prove-over-all-inputs and the teeth direction).
- `ripple_carry_sum` — a symbolic `(a+b) mod 2^n` reference for stating claims.
"""
from .refspec import (
    add_bits,
    bits_eq,
    const_bits,
    mod_add,
    mod_double_canonical,
    mod_reduce_once,
    mod_sub,
    sub_bits,
    ult,
)
from .symsim import (
    OpStream,
    PHASE_OP_KINDS,
    SymSim,
    SymState,
    find,
    load_streams,
    prove,
    replay,
    require_proved,
    require_teeth,
    ripple_carry_sum,
)

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
    # refspec — independent reference arithmetic for stating claims
    "const_bits",
    "add_bits",
    "sub_bits",
    "ult",
    "bits_eq",
    "mod_add",
    "mod_sub",
    "mod_double_canonical",
    "mod_reduce_once",
]
