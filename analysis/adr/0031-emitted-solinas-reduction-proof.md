# ADR 0031 — Prove the Solinas reduction over the emitted `mod_add_qq` gates (referee F2)

**Status:** Accepted — implemented in `analysis/verify/solinas_reduction_emitted.py`
(z3, via `proof_toolkit`) over the emitted op-stream dumped by
`src/point_add/modadd_dump.rs` (`#[cfg(test)]`) into `analysis/mod_add_qq_ops.json`.
Wired as `just solinas-emitted`. Closes the copy↔emitter gap (F2) for the modular
**reduction** wrapper, the last piece still covered only by re-implementations.
Analysis-layer only; the scored circuit is byte-identical (`ops.bin` SHA `f30d8365…`).
**Date:** 2026-07-03

## Context

Two earlier layers bound the emitted circuit to its proofs — ADR 0027 (z3 replay of
the emitted `cuccaro_add_fast`) and ADR 0030 (Kani driving the real builder+simulator
for the same adder). Both cover the **addition primitive**. The **Solinas modular
reduction** on top of it (`mod_add_qq`: add `c = 2^256 − p`, branch on the overflow
bit, undo the add or clear the bit, uncompute the flag) was still proven only by
*re-implementations*:

- `analysis/verify/solinas_reduction.py` — z3 that mirrors the algorithm
  "step-for-step" in BitVec; a faithful *model*, not the emitted gates.
- `src/kani_proofs.rs::solinas_add` — a hand-written integer *twin* of the control
  flow; the exact copy referee finding **F2** warns about.

If the gate emitter drifts from the model/twin, both stay green. This ADR removes that
last gap by proving the reduction over the gates the builder actually emits.

## Decision

Reuse the ADR 0027 "prove-what-you-emit" pattern — now on the generalized
`proof_toolkit` replayer (ADR 0029) — for the whole `mod_add_qq` wrapper.

1. **Dump the real op-stream** (`src/point_add/modadd_dump.rs`, `#[cfg(test)]`). The
   real `B` builder emits `mod_add_qq` at the production 256-bit secp256k1 width
   (7 216 ops: CX/CCX/X plus 521 `R` resets from freeing ancilla) to
   `analysis/mod_add_qq_ops.json`. A non-ignored drift-guard test
   (`emitted_mod_add_qq_matches_committed_artifact`) fails loudly if the committed
   artifact goes stale against a fresh emit — the drift guard F2 asks for. (Only 256
   is dumped: `mod_add_qq` bakes `c = 2^256 − p`, so it is correct only at `n = 256`.)

2. **Replay symbolically** (`analysis/verify/solinas_reduction_emitted.py`). The stream
   runs through the `proof_toolkit` z3 model of `src/sim.rs` with `acc`/`a` free and
   each `R`-reset outcome a fresh free boolean. It proves, over **all** `acc, a ∈ [0, p)`
   (added as z3 preconditions) and all outcomes:
   - **functional** `acc' == (acc + a) mod p`, against an *independent* ripple-carry-add
     + conditional-subtract-p reference (textbook reduction, structurally unlike the
     Solinas +c/overflow path being replayed);
   - **a-preserved**;
   - **clean** — the flag qubit and every ancilla (const-load registers, carries,
     ext-overflow bits) return to |0>;
   - **phase-clean** — net phase 0 for every outcome. Because the only phase-bearing
     ops are the `R` resets (`phase ^= q·m`), phase-clean is exactly the statement that
     every ancilla is genuinely |0> at the moment the circuit resets it.

## As built

`just solinas-emitted`. z3 discharges the 256-bit stream `unsat` on the negated claims
in ≈4.7 min — a genuine proof over 7 216 emitted gates and 521 free reset outcomes, but
far heavier than the ~2 s z3 stages, so (like `just kani`) it is kept **out** of the
default `just analysis` suite and run explicitly. The proof consumes the byte-identical
committed artifact; the Rust drift guard (`emitted_mod_add_qq_matches_committed_artifact`)
runs in the normal `cargo test` job.

## Consequences

- **Closes F2 for the reduction, on both the adder and the wrapper.** Every emitted
  modular-add primitive is now bound to a proof over the *emitted gates* — the adder in
  z3 (0027) and Kani (0030), and now the full Solinas reduction in z3.
- **The model/twin become cross-checks, not the sole evidence.** `solinas_reduction.py`
  (step-for-step BitVec) and `kani_proofs.rs::solinas_add` (integer twin) still run as
  independent confirmations, but the load-bearing proof no longer depends on a
  re-implementation matching the emitter.
- **Honest scope.** This proves `mod_add_qq` (the plain Solinas reduction, the 3
  non-fast calls). The `_fast` wrappers fold the same reduction around the measurement-
  based adder proved in ADR 0027; a symbolic execution of the *whole composed*
  point-add remains the standing z3-intractable stretch (`scientific-value.md` §4),
  unchanged. `c = 2^256 − p` fixes the width at 256, so there is a single width, not the
  2..256 sweep of the adder proof.
- **Reuses the toolkit, validating ADR 0028/0029's thesis.** A second consumer of the
  `proof_toolkit` replayer (beyond `mbuc_phase_correction.py`) — the "prove-what-you-
  emit" methodology applied to a new primitive with only a new dump harness and a new
  claim script, no re-derivation of sim semantics. The `prove()` helper gained an
  `assumptions` parameter for the `a, acc < p` precondition.
- **Isolation ([ADR 0001](0001-analysis-layer-isolated-from-score.md)).** A `verify/`
  z3 script + a `#[cfg(test)]` dump/guard; never compiled into `build_circuit`. The
  scored secp256k1 circuit is byte-identical (`ops.bin` SHA `f30d8365c1235002`).
