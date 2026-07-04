# ADR 0036 — Prove the scored circuit's adder over its emitted gates

**Status:** Accepted — implemented in `analysis/verify/scored_add_emitted.py` (z3, via
`proof_toolkit`) over the emitted op-streams dumped by
`src/point_add/trailmix_ludicrous/scored_add_dump.rs` (`#[cfg(test)]`) into
`analysis/scored_add_ops.json`. Wired as `just scored-add`, in the default `just analysis`
suite (~5 s). The first emitter-bound proof of a gate family the **scored** `ops.bin`
actually runs, closing the gap ADR 0035 named.
**Date:** 2026-07-04

## Context

[ADR 0035](0035-proof-scope-scored-vs-reference.md) disclosed that the emitter-bound
arithmetic proofs (ADR 0027/0030/0031/0032) bind *reference* primitives
(`arith/modular/*_fast`) which `point_add::build()` does not emit; the scored circuit is
`trailmix_ludicrous`, with its own arithmetic. It named the tractable next step: bind a
proof to `trailmix_ludicrous`'s own adder and comparator.

`arith::hybrid_add_adaptive(circ, a, b, k)` is the adder the scored square's `add_into`
drives (`square.rs`): `a := (a + b) mod 2^n`, `b` preserved. It dispatches on headroom `k`
to either a plain Gidney measurement-vented add or a sqrt(n)-chunked add whose boundary
carries are measurement-vented (HMR + CZ/Z/NEG fixups) and gated by
`PUSH_CONDITION`/`POP_CONDITION`. It is deterministic in `(n, k)` (layout from
`adaptive_layout(n,k)`; no `active_qubits` read), so a fresh-builder emit reproduces the
scored gates for that `(n, k)`.

## Decision

Apply the ADR 0027 "prove-what-you-emit" pattern to the scored adder.

1. **Dump** (`scored_add_dump.rs`, `#[cfg(test)]`) `hybrid_add_adaptive` at nine `(n, k)`
   configs spanning **both** dispatch branches — plain (`k + 2√n ≥ n`) and chunked — at
   widths 4/8/16/64/128/**256**, into `analysis/scored_add_ops.json`. A drift-guard test
   (`emitted_scored_add_matches_committed_artifact`) keeps the artifact byte-identical to a
   fresh emit; it runs in `cargo test` / CI.
2. **Replay + prove** (`scored_add_emitted.py`) through the `proof_toolkit` z3 model, the
   HMR/`R` outcomes free/∀, proving each config (each claim group its own solve):
   - **functional** `a' == (a + b) mod 2^n` (against `ripple_carry_sum`);
   - **b-preserved**;
   - **clean** — every ancilla (vented boundary carries, the zero-pad) returns to |0>;
   - **phase-clean** — net phase 0 for **all** measurement outcomes.

## As built

`just scored-add` proves all nine configs `unsat` on the negation in **≈5 s** total —
including the chunked 256 config (6 241 ops, 921 free measurement outcomes) whose
`PUSH/POP_CONDITION` + `NEG`/`Z` measurement-vented carry uncompute is proved phase-clean.
Light enough to sit in the default `just analysis` suite (unlike the minutes-long
`solinas-emitted` / `mod-fast-emitted`).

## Consequences

- **First proof bound to the *scored* gates.** Upgrades the emitter-bound arc from
  "proved a reusable reference sibling" (ADR 0035's caveat) to "proved the scored
  circuit's core arithmetic adder over the gates `ops.bin` runs" — both dispatch branches,
  at production width, over all inputs and all measurement outcomes.
- **Exercises new op territory faithfully.** The scored adder's `PUSH/POP_CONDITION`
  classically-conditioned vent regions and `NEG`/`Z` phase fixups are proved phase-clean —
  the toolkit's condition-stack and phase-op modelling verified against a real scored
  primitive, not just the reference adders.
- **Honest remaining scope.** The scored **comparator** (`compare_geq_cin_middle`) is
  *higher-order* (a `body: FnOnce` closure) and *stateful* (`next_compare_cin_call_index`),
  so it is not isolatable the way the adder is — deferred (issue #79). The scored **Kaliski
  inverse** / squaring at 256 and the **composed** point-add remain z3/BMC-intractable and
  sampled (the standing wall). This ADR binds the adder, not the whole scored circuit.
- **Isolation ([ADR 0001](0001-analysis-layer-isolated-from-score.md)).** A `verify/`
  script + a `#[cfg(test)]` dump/guard inside `trailmix_ludicrous`; never compiled into
  `build_circuit`. The scored circuit is byte-identical (`ops.bin` SHA `f30d8365c1235002`).
