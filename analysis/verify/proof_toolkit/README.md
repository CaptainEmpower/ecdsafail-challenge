# `proof_toolkit`

The repo's verification **methodology** as a small, reusable Python library: prove a
property of the circuit you actually **emit** by replaying its op-stream through a
faithful z3 model of the simulator that scores it — over *all* inputs and *all*
measurement outcomes.

It is the extracted core of the ADR 0027 proof, generalized so any emitted op-stream
can be verified the same way. Scope and rationale live in the ADR trail
([0028](../../adr/0028-reusable-proof-toolkit.md) — what is/ isn't reusable,
[0029](../../adr/0029-proof-toolkit-op-stream-replayer.md) — this build); the *why* of
the design is in [`DESIGN.md`](DESIGN.md). This file is the practical how-to.

Analysis-layer only ([ADR 0001](../../adr/0001-analysis-layer-isolated-from-score.md)):
nothing here compiles into the scored circuit.

## What's in the box

| File | Role |
|---|---|
| `symsim.py` | The library. `SymSim`/`replay` (z3 model of every `src/sim.rs` op), `OpStream`/`load_streams` (dump loader), `prove`/`find` + assertion helpers, `ripple_carry_sum`. |
| `__init__.py` | Public API re-exports (import from `proof_toolkit`). |
| `selftest.py` | `just toolkit` — pins each op's symbolic semantics against hand-computed truth, artifact-independent. |
| `__main__.py` | `python -m proof_toolkit` → runs the self-test. |

## Consumers (worked examples to copy)

- [`../mbuc_phase_correction.py`](../mbuc_phase_correction.py) — the emitted `_fast`
  adder's HMR + `cz_if` phase correction ([ADR 0027](../../adr/0027-mbuc-phase-correction-proof.md)).
- [`../solinas_reduction_emitted.py`](../solinas_reduction_emitted.py) — the emitted
  `mod_add_qq` Solinas reduction ([ADR 0031](../../adr/0031-emitted-solinas-reduction-proof.md)).

Each pairs with a Rust `#[cfg(test)]` dump harness that emits the real op-stream and a
drift-guard test keeping the committed JSON byte-identical to a fresh emit
(`src/point_add/mbuc_dump.rs`, `src/point_add/modadd_dump.rs`).

## Running

```bash
uv sync --locked                         # z3, pinned (repo root)
uv run just toolkit                      # self-test (fast)
uv run just mbuc                         # ADR 0027 proof (~2 s)
uv run just solinas-emitted              # ADR 0031 proof (~5 min, 256-bit)
# or directly, from analysis/verify/:
python -m proof_toolkit.selftest
```

## The op-stream format

A stream bundle is JSON `{"widths": [ {<metadata>, "ops": [ <op>, ... ]}, ... ]}`, as
emitted by the Rust dump harnesses' `ops_to_json`. Each **op** is a 6-tuple

```
[kind, q_control2, q_control1, q_target, c_target, c_condition]
```

where `kind` is the `OperationType` name (`"CX"`, `"CCX"`, `"HMR"`, `"SWAP"`, …) and
qubit/bit ids are non-negative ints, `-1` for absent (`NO_QUBIT`/`NO_BIT`).
`load_streams(path)` returns one `OpStream` per `widths` entry, splitting `ops` from the
per-stream metadata (reachable via `stream["n"]`, `stream.get(...)`).

Modelled op kinds (faithful to `src/sim.rs::apply_iter`): `CX CCX X SWAP CZ Z NEG CCZ
HMR R BIT_INVERT BIT_STORE0 BIT_STORE1 PUSH_CONDITION POP_CONDITION` (plus
`REGISTER`/`APPEND_TO_REGISTER`/`DEBUG_PRINT` as no-ops). An unmodelled kind raises.

## Public API

```python
from proof_toolkit import (
    replay, SymSim, SymState, OpStream, load_streams,
    prove, find, require_proved, require_teeth,
    ripple_carry_sum, PHASE_OP_KINDS,
)
```

**`replay(ops, *, qubit_inputs=None, bit_inputs=None, drop_phase_kinds=frozenset(), meas_prefix="m") -> SymState`**
Execute `ops` from a fresh symbolic state.
- `qubit_inputs` / `bit_inputs`: `{id: z3.Bool}` seeding the free-input registers.
  Everything else starts `|0>` / `False`.
- each `HMR`/`R` measurement outcome becomes a **fresh free `Bool`**, collected in
  `SymState.meas` — a claim proved over them holds for *every* outcome.
- `drop_phase_kinds` (⊆ `PHASE_OP_KINDS`) suppresses the phase contribution of the
  named phase ops — the **teeth** lever (show a correction is load-bearing).

**`SymState`**: `.qubits`, `.bits` (`{id: expr}`), `.phase`, `.meas`; helpers `.q(id)`,
`.bit(id)` (return `|0>`/`False` for untouched ids).

**`load_streams(path) -> list[OpStream]`**; `OpStream.ops`, `OpStream.meta`,
`stream[key]`, `stream.get(key, default)`.

**`prove(claims, assumptions=()) -> z3 result`** — `unsat` means proved (no input /
outcome satisfying `assumptions` violates any claim). `find(expr)` — `sat` means a
witness exists (teeth direction). `require_proved(claims, label, assumptions=())` /
`require_teeth(expr, label)` wrap these and raise `SystemExit` on the wrong result.

**`ripple_carry_sum(a_bits, b_bits) -> list`** — symbolic `(a+b) mod 2^n`, a shared
reference for stating adder claims.

## Writing a new op-stream proof

1. **Dump** the primitive from the real builder in a Rust `#[cfg(test)]` harness
   (copy `modadd_dump.rs`), with a byte-identical drift-guard test.
2. **Load & replay**, seeding the free-input registers:

   ```python
   from z3 import Bool, BoolVal
   from proof_toolkit import load_streams, replay, require_proved, ripple_carry_sum

   stream = load_streams("../my_ops.json")[0]
   n, xs, ys = stream["n"], stream["x"], stream["y"]
   x_in = [Bool(f"x{i}") for i in range(n)]
   inputs = {qid: v for qid, v in zip(xs, x_in)}          # other qubits ⇒ |0>
   st = replay(stream.ops, qubit_inputs=inputs)
   ```
3. **State claims** against an *independent* reference and prove:

   ```python
   ref = ripple_carry_sum(x_in, ...)                      # your spec, not the impl
   ancilla = [q for q in st.qubits if q not in xs]
   claims  = [st.q(xs[i]) == ref[i] for i in range(n)]    # functional
   claims += [st.q(q) == BoolVal(False) for q in ancilla] # clean
   claims += [st.phase == BoolVal(False)]                 # phase clean
   require_proved(claims, f"n={n}")
   ```
4. **Add teeth**: re-`replay` with `drop_phase_kinds=...` (or a mutated stream) and
   `require_teeth(st_broken.phase == BoolVal(True), "teeth")` to confirm the correction
   you're relying on actually bites.
5. **Wire** a `just` recipe + an ADR; keep heavy (256-bit, minutes-long) proofs out of
   the default `just analysis` suite.

See `DESIGN.md` for why the model is shaped this way (single symbolic shot, free
outcomes, drift guards, independence of the reference).
