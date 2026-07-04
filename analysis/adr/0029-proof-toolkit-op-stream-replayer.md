# ADR 0029 — Build the proof toolkit's first module: the generalized z3 op-stream replayer

**Status:** Accepted — implemented in `analysis/verify/proof_toolkit/` (a Python
package: `symsim.py` + `selftest.py`), the first extracted module of the toolkit scoped
in [ADR 0028](0028-reusable-proof-toolkit.md). Wired into the analysis suite as
`just toolkit` (self-test) and consumed by `mbuc_phase_correction.py` (ADR 0027).
Analysis-layer only; the scored circuit is byte-identical (`ops.bin` SHA `f30d8365…`).
**Date:** 2026-07-03

## Context

[ADR 0028](0028-reusable-proof-toolkit.md) settled *what* is reusable about this repo:
**not** the hand-tuned scored gates (score-specialized, single-consumer, and bound by the
byte-identical `ops.bin` constraint — carving them out is risk for no payoff) but the
**verification methodology** — replay the op-stream you actually emit through a faithful
symbolic model of the simulator that scores it, and prove a property over all inputs and
all measurement outcomes. It stayed **Proposed**, and named the promotion path explicitly:

> A follow-up issue/ADR would promote it to Accepted and scope the toolkit's first
> extracted module (most likely the generalized z3 op-stream replayer, since ADR 0027
> already produced its core).

That core lived as a private `replay()` inside `mbuc_phase_correction.py` (ADR 0027): a z3
model of a *subset* of `src/sim.rs` ops (CX/CCX/X/CZ/Z/CCZ/NEG/HMR/R), hard-wired to the
one adder stream it proved, with a bespoke `drop_cz` teeth flag. Reusable in spirit, but
not reusable in fact — a second op-stream proof would have to copy and re-adapt it.

## Decision

Promote ADR 0028's first module and build it: extract the replayer into a standalone,
op-stream-agnostic package, `analysis/verify/proof_toolkit/`, and make the existing proof
its first consumer.

1. **`proof_toolkit.symsim` — the generalized replayer.** `SymSim` mirrors
   `Simulator::apply_iter` (`src/sim.rs`) op-for-op, now covering **every** op kind the
   emitter can produce — the ADR 0027 set plus `Swap`, `BIT_INVERT`/`BIT_STORE0`/
   `BIT_STORE1`, and the `PUSH_CONDITION`/`POP_CONDITION` base-condition stack — so any
   emitted stream replays, not just the fast adder. One symbolic shot (z3 `Bool` per
   qubit / classical bit, one `Bool` global phase) models all 64 parallel u64-masked shots
   since they are independent; each `HMR`/`R` outcome is a **fresh free `Bool`** (∀
   outcomes, not the random XOF). `replay(ops, qubit_inputs=…, bit_inputs=…)` returns a
   `SymState` (`.q(id)`, `.bit(id)`, `.phase`, `.meas`); `load_streams()` parses the
   `{"widths":[…]}` dump format (`src/point_add/mbuc_dump.rs`); `prove`/`find`/
   `require_proved`/`require_teeth` wrap the prove-over-all-inputs and teeth directions;
   `ripple_carry_sum` is a shared reference for stating adder claims. The teeth lever is
   generalized from `drop_cz` to `drop_phase_kinds ⊆ {CZ, Z, CCZ, NEG}`.

2. **`proof_toolkit.selftest` — the toolkit's own teeth (`just toolkit`, `python -m
   proof_toolkit.selftest`).** Nine cases pin each op's symbolic semantics against
   hand-computed truth on tiny streams z3 settles instantly (CX/X, Toffoli-AND, Swap,
   CZ/CCZ phase, the HMR+`cz_if` phase-cancellation *with* a dropped-CZ teeth `sat`, the
   condition stack, classical-bit stores, the ripple-carry reference, and an unmodeled-op
   guard). This gives the replayer coverage **independent of any dumped artifact**, so a
   regression in the op model is caught even if every committed op-stream is stale.

3. **Wire the existing proof to it.** `mbuc_phase_correction.py` now imports the toolkit
   and only states the adder-specific claims (functional = `ripple_carry_sum`, registers
   clean, phase 0; teeth = drop the `cz_if` CZ). Its private `replay`/`expected_sum_bits`
   are deleted. Output is byte-identical to the ADR 0027 version (same `[PROVED]` lines,
   same per-width measurement counts, teeth firing at every width incl. production 256).

## As built

`just toolkit` (15th analysis stage, run first so the shared replayer is self-tested
before the proofs that drive it) prints all nine `[OK]` cases; `just mbuc` is unchanged in
output but now runs on the package. Both discharge in ≈2 s on the locked z3. `just pycheck`
byte-compiles the package with the rest of the analysis layer on the 3.11 floor.

## Consequences

- **The reusable asset now exists in fact, not just in principle.** The "prove-what-you-
  emit" pattern — the transferable science ADR 0028 named — is a library any future
  op-stream proof imports, rather than a copy-me snippet inside one script.
- **Faithful to the whole simulator, not one adder.** Modelling every `src/sim.rs` op kind
  (conditions and classical bits included) means the next consumer — e.g. a Kani harness
  bound to the emitter, or a symbolic check of a composed sub-circuit
  (`ROADMAP.md` open stretches) — replays without re-deriving semantics.
- **No new claim, no widened scope.** This is a refactor-plus-generalize of proven code:
  the mbuc proof still covers exactly the `cuccaro_add_fast` primitive it did under ADR
  0027 (the honest remaining scope in that ADR — no symbolic execution of the full composed
  point-add — is unchanged). The toolkit is plumbing for future proofs, not itself a proof
  of anything new about the circuit.
- **Consistent with the deferrals ADR 0028 recorded.** Only the methodology module is
  built; the clean-room *primitive* crate stays deferred until a second consumer exists.
  Nothing was carved out of `src/point_add/`; the drift-guard dump (`mbuc_dump.rs`) and the
  reference/Kani bridges are unchanged.
- **Isolation ([ADR 0001](0001-analysis-layer-isolated-from-score.md)).** A `verify/`
  Python package + a self-test recipe; never compiled into `build_circuit`. The scored
  secp256k1 circuit is byte-identical (`ops.bin` SHA `f30d8365c1235002`, unchanged).
