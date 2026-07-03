# ADR 0028 — Package the verification methodology as a reusable proof toolkit (design-only, issue #70)

**Status:** Proposed — a design record scoping how (and how *not*) to make the repo's
proven results reusable. Docs-only; nothing is built. Isolated from the scored circuit
([ADR 0001](0001-analysis-layer-isolated-from-score.md)); `ops.bin` untouched.
**Date:** 2026-07-03

## Context

Over ADRs 0001–0027 the repo accumulated a substantial, machine-checked verification
layer, and a natural question followed: *should the proved lemmas be cleaned out of the
Rust and made available as a reusable library?*

Two things pull apart under that question:

- **The scored circuit** (`src/point_add/`) is a hand-tuned artifact whose sole purpose is
  to minimize `round(avg_toffoli) × qubits`. Its correctness is anchored by the proofs, but
  the *code* is written to win a score, not to be a library.
- **The proofs / methodology** already live outside the scored circuit — z3 in
  `analysis/verify/*.py`, Kani in `src/kani_proofs.rs`, the reference-adder validator in
  `verify/kickmix_sim.py`, and (ADR 0027) a z3 replayer of the *emitted* op-stream with a
  byte-identical drift guard (`mbuc_phase_correction.py` + `src/point_add/mbuc_dump.rs`).

The referee framing (`scientific-value.md`) is deliberate: the deliverable is not a break
but a **standard of evidence** — a template showing that "X qubits, Y Toffoli" estimates
*can* be machine-checked and completeness-verified without giving up competitiveness. That
framing points at what is actually reusable.

## Decision (proposed)

**1. Do not carve the lemmas / primitives out of the scored `src/point_add/` into a
library.** Three reasons:

- **Byte-identical constraint.** The deliverable is `ops.bin` at SHA `f30d8365…`. Even
  pure code movement within the crate ([#10](https://github.com/CaptainEmpower/ecdsafail-challenge/issues/10))
  must be re-validated byte-identical after every step; extracting primitives *out* into a
  crate is strictly riskier, for a speculative payoff (there is exactly one consumer today).
- **Most of the hot path is score-specialized, not general** (`scientific-value.md §3`
  already separates these): Solinas folding is bespoke to `c = 2^32 + 977`; the fused
  double / symmetric square-subtract depend on the `a=0, b=7` group law; the baked
  `trailmix_ludicrous` schedules are curve/harness-specific. The genuinely reusable
  primitives (Cuccaro adder, measurement-based vented uncompute, Kaliski two-inverse
  conjugate-uncompute, sound constant-propagation peephole) are entangled with the `B`
  builder, the `Op` type, and the scoring `Simulator`.
- **The lemmas are already extracted**, and they — not the Rust primitives — are the
  portable artifact. A Rust primitive crate would not carry the z3/Kani proofs with it in
  any enforced way, so it would be *less* rigorous than what exists.

**2. The reusable package is the *verification methodology*, not the scored primitives.**
If reusability is pursued, do it as a **verified-reversible-arithmetic proof toolkit**:

- the z3 `sim.rs`-semantics replayer (generalized from `mbuc_phase_correction.py`): given
  an emitted op-stream and a claimed property, prove it over **all inputs and all
  measurement outcomes** (the ADR 0027 pattern);
- the **emitted-op dump + byte-identical drift-guard** pattern (`mbuc_dump.rs`): the proof
  verifies what you *run*, not a re-implementation (the F2 concern, generalized);
- the reference-adder validator (`kickmix_sim.py`) and the Kani ↔ real-`U256` bridge.

This lives in the analysis layer (or a sibling `verify/` package), is backend-agnostic
where the op-stream format allows, and is **decoupled from the score** so the byte-identical
constraint is never at risk.

**3. Deferred alternative — a clean-room primitive crate.** An actual verified primitive
library (Cuccaro + vent + Kaliski + constprop, backend-abstracted via a trait, primitives
re-derived in *general* form with z3/Kani proofs wired into its own CI) is recorded as a
possible **separate clean-room** effort — explicitly *not* carved out of `src/point_add/`.
It is deferred until a second consumer exists; today it would be premature generalization.

## Consequences

- **Preserves the invariant that matters.** The scored circuit and `ops.bin` are never put
  at risk by a reusability refactor.
- **Names the reusable asset correctly.** The transferable science is the *how-to-verify*
  methodology (symbolic sim-semantics replay, prove-what-you-emit drift guards,
  reference-artifact cross-validation, model→real-type bridging), which the repo already
  embodies — not a fork of the hand-tuned gates.
- **Keeps options open without over-committing.** The primitive-crate path is documented as
  a deferred alternative with an explicit trigger (a second consumer), rather than silently
  dropped.
- **No build yet.** This ADR stays **Proposed**. A follow-up issue/ADR would promote it to
  Accepted and scope the toolkit's first extracted module (most likely the generalized z3
  op-stream replayer, since ADR 0027 already produced its core).
- **Isolation.** Design-only, docs-only; `#[cfg(test)]`/analysis-layer if ever built; the
  scored secp256k1 circuit is byte-identical (`ops.bin` SHA `f30d8365c1235002`).
