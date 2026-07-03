# ADR 0027 — z3 proof of the emitted `_fast` adder's measurement-based uncompute (referee F1/F2)

**Status:** Accepted — implemented in `analysis/verify/mbuc_phase_correction.py` (z3 proof)
over the emitted op-stream dumped by `src/point_add/mbuc_dump.rs` (`#[cfg(test)]`) into
`analysis/mbuc_fast_adder_ops.json`. Delivers the *optional stretch* named in referee
finding **F1/F2** (`paper/REVIEW.md`, [ADR 0023](0023-external-referee-review.md)); the
disclosure half of F1/F2 is handled separately in the honest-scope framing work (ADR 0026).
**Date:** 2026-07-03

## Context

The referee review found a genuine **coverage gap between the proofs and the emitted
circuit** ([ADR 0023](0023-external-referee-review.md), findings F1/F2):

- The scored hot path emits `mod_add_qq_fast` / `mod_sub_qq_fast` /
  `mod_double_inplace_fast` — **58 fast calls vs 3 plain `mod_add_qq`** — all routing their
  addition through `cuccaro_add_fast` (`src/point_add/arith/adder.rs`). Its backward UMA
  sweep clears each carry ancilla with **Gidney measurement-based uncomputation**:
  `b.hmr(carry, m)` (X-basis measure → random outcome `m`, qubit reset to `|0>`, phase
  kickback `carry·m`) then `b.cz_if(x, y, m)` (a classically-conditioned CZ applying phase
  `m·x·y`). Zero Toffoli — but correctness rests on the two phase terms **cancelling**,
  `carry·m ⊕ x·y·m = 0`, for *every* measurement outcome, which requires `x·y` to still
  equal the AND that produced `carry` at uncompute time.
- The existing formal layer (`solinas_reduction.py` z3, `src/kani_proofs.rs`) models only
  the **plain** `mod_add_qq` and treats adders as exact integer `+`. So the HMR/`cz_if`
  phase-kickback logic the emitted circuit actually runs has **zero symbolic coverage** —
  guarded only by the 9024-shot sample. F2 sharpens this: the Kani harness proves a
  *hand-written copy* on plain integers, so if the copy and the emitter drift, the proof
  stays green.

The referee framed F1/F2 as *disclosure, not necessarily new proof*, with an explicit
**optional stretch: "a z3/sim model of the HMR + `cz_if` phase correction."** This ADR
delivers that stretch.

## Decision

Prove the emitted `_fast` adder's measurement-based uncompute in z3, **over the actually
emitted gates**, with the measurement outcomes free (universally quantified):

1. **Dump the real op-stream** (`src/point_add/mbuc_dump.rs`, `#[cfg(test)]`). The real
   `B` builder emits `cuccaro_add_fast` at widths `{2,3,4,8,16,64,256}` (incl. the
   production 256-bit coordinate width); the ops are serialized to
   `analysis/mbuc_fast_adder_ops.json`. **Emitting from the real builder is the drift
   guard** F2 asks for — the proof verifies the emitter's output, not a re-implementation.
   A non-ignored `#[cfg(test)]` test (`emitted_ops_match_committed_artifact`) fails loudly
   if the committed artifact ever goes stale against a fresh emit.
2. **Replay symbolically in z3** (`analysis/verify/mbuc_phase_correction.py`). A z3 model
   of `src/sim.rs`'s per-op semantics (X/CX/CCX/CZ/Z/CCZ/HMR/R, condition bits) executes
   the op-stream with `a`/`acc` free boolean inputs, `c_in`/carries at `|0>`, and each HMR
   outcome a **fresh free boolean** `m_k` (∀ outcomes, not the random XOF). It proves, by
   `unsat` on the negation:
   - **functional** `acc' == (a + acc) mod 2^n`;
   - **clean** `a` unchanged, `c_in` and every carry ancilla back to `|0>`;
   - **phase-clean** net global phase `== 0` for **all** inputs and **all** measurement
     outcomes — the HMR kickback `carry·m` is exactly cancelled by `cz_if`'s `x·y·m`.
3. **Teeth.** Dropping the `cz_if` corrections makes the phase claim *fail* (z3 returns
   `sat` — an input+outcome with net phase 1), confirming the correction is load-bearing.

## As built

`just mbuc` (14th analysis stage). z3 discharges every width — including the production
256 — in ≈2 s total, all `unsat` on the negated claims, teeth firing at each width. The
proof consumes the byte-identical committed artifact; the Rust drift guard runs in the
normal `cargo test` job.

## Consequences

- **Closes the F1/F2 verification gap on the emitted fast adder.** The measurement-based
  uncompute the scored circuit runs is now proven correct and phase-clean over all inputs
  and all measurement outcomes at production width — no longer sampled-only. Because every
  `_fast` call routes its addition through `cuccaro_add_fast`, the shared MBUC primitive is
  the load-bearing piece, and it is now covered.
- **Verifies the emitter, not a copy — the specific F2 concern.** The proof runs on the
  op-stream the real `B` builder emits, with a drift guard binding the committed artifact
  to a fresh emit, so a divergence between "the copy" and the emitted gates cannot pass
  silently.
- **Complements, does not replace, the F1/F2 disclosure (ADR 0026).** The honest-scope
  framing still states plainly what each proof covers; this ADR moves the MBUC phase
  correction from *disclosed-but-unproven* to *proven*.
- **Honest remaining scope.** This proves the `cuccaro_add_fast` **primitive** (the shared
  MBUC carry-clear), not a symbolic execution of the whole composed 256-bit point-add
  against the reference group law — that full composition remains guarded by the sampled
  end-to-end check (the standing stretch in `scientific-value.md §4`). The modular wrappers
  (`mod_add_qq_fast` etc.) add classical-constant folds around this same proven primitive.
- **Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md).** Analysis
  layer only — a `verify/` z3 script + a `#[cfg(test)]` dump/guard; never compiled into
  `build_circuit`. The scored secp256k1 circuit is byte-identical (`ops.bin` SHA
  `f30d8365c1235002`, unchanged).
