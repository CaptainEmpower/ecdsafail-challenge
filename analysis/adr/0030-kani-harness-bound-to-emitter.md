# ADR 0030 — Kani harness bound to the emitter, not a copy (referee F2)

**Status:** Accepted — implemented in `src/point_add/mbuc_kani.rs`
(`#[cfg(any(test, kani))]`): two `#[kani::proof]` harnesses driving the real `B`
builder + real `Simulator`, plus an exhaustive `#[cfg(test)]` shadow that runs in the
normal `cargo test`. Wired into `analysis/verify/run_kani.sh`. Closes the optional
copy↔emitter Kani stretch named in the ROADMAP and referee finding **F2**
(`paper/REVIEW.md`). Analysis/test-layer only; the scored circuit is byte-identical
(`ops.bin` SHA `f30d8365…`).
**Date:** 2026-07-03

## Context

The referee's finding **F2** is about *what the proof is bound to*. The existing
Kani harnesses (`src/kani_proofs.rs`, ADR 0024) prove `solinas_add` — a hand-written
integer twin of `mod_add_qq`'s control flow. That is rigorous about the *arithmetic*,
but it is a **copy**: if the gate-emitting builder and the twin drift, a proof over
the twin stays green. F2 asks for a proof bound to the emitter itself.

ADR 0027 closed the *phase* half of that gap in **z3**, replaying the actually
emitted `cuccaro_add_fast` op-stream. But the z3 side is still a *model* of
`src/sim.rs` written in Python (`proof_toolkit`, ADR 0028/0029). The remaining gap is
Rust-native: a proof that drives the **real builder and the real simulator** — the
same code the scorer runs — rather than any re-implementation.

## Decision

Add a Kani harness that emits and simulates the real gates, over free inputs **and**
free measurement outcomes.

1. **Drive the real emitter + real simulator.** `drive_and_check(n, a, acc, reader)`
   (`src/point_add/mbuc_kani.rs`) calls the real `B::new_for_test()` /
   `alloc_qubits` / `cuccaro_add_fast` (`arith/adder.rs`) to emit the gate stream,
   then runs the real `Simulator` (`src/sim.rs`) on it — no twin. It asserts the full
   contract on shot 0: **functional** `acc' == (a + acc) mod 2^n`, **a-preserved**,
   **ancilla-clean** (`c_in` and every carry ancilla back to |0>), and **phase-clean**
   (net phase bit 0 == 0 — the HMR kickback exactly cancelled by the `cz_if` fixup).

2. **Free measurement outcomes.** The simulator's 64 shots are bitwise-independent, so
   populating only shot 0 and asserting on bit 0 models a single shot with a free
   outcome. Under Kani the HMR randomness comes from an `XofReader` returning a free
   bit per read, so the harness proves the contract for **all inputs and all
   measurement outcomes** — the same ∀ guarantee as ADR 0027, now on real Rust types.

3. **One body, two drivers.** The `#[kani::proof]` harnesses (`mbuc_fast_adder_width2`,
   `…_width3`) supply symbolic inputs + symbolic outcomes; an exhaustive
   `#[cfg(test)]` shadow (`mbuc_kani::shadow`) supplies *every* input and *every*
   outcome at widths 2/3/4 through the same `drive_and_check`. The shadow runs in the
   normal `cargo test` job, so the harness is exercised — and guarded against bit-rot —
   even where `cargo kani` is not installed.

## As built

`bash verify/run_kani.sh` (or `just kani`) now runs `mbuc_fast_adder_width2/3`
alongside `solinas_add_u64/u256`. The shadow test
(`point_add::mbuc_kani::shadow::mbuc_fast_adder_exhaustive_shadow`) passes in
`cargo test --release` — exhaustive over all inputs and all measurement outcomes at
widths 2/3/4 (2 048 concrete runs of the real emitter+simulator).

## Consequences

- **Closes the copy↔emitter gap on the Rust side (F2).** The proof is bound to the
  gates the builder emits and the simulator that scores them, not a re-implementation.
  Together with ADR 0027 (z3 over the emitted op-stream) and ADR 0024 (real `U256`
  types), the emitted `_fast` adder is now covered by an emitter-bound proof in **both**
  solvers.
- **Honest scope.** Kani discharges this bit-precisely at **small width** (2/3); the
  shadow adds concrete width 4. This does **not** prove production width 256 in Kani —
  BMC does not scale to the 256-bit adder (that is exactly what the z3 layer covers,
  ADR 0027, all-width abstract). The value here is *binding to the real emitter/types*,
  not width. The composed full point-add remains the standing z3-intractable stretch
  (`scientific-value.md` §4), unchanged.
- **No new claim, no score impact.** A refactor-free addition of proof code behind
  `#[cfg(any(test, kani))]`; never compiled into `build_circuit`. The scored secp256k1
  circuit is byte-identical (`ops.bin` SHA `f30d8365c1235002`, unchanged).
- **Complements ADR 0028/0029.** The Python `proof_toolkit` replays the emitted
  op-stream through a *model* of the simulator; this Kani harness drives the *actual*
  simulator. The two are independent bindings of the same emitted primitive — a model
  bug in one would not mask a real bug, since the other executes the real code.
- **Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md).** Test/proof
  layer only, `#[cfg(any(test, kani))]`; the scored circuit is untouched.
