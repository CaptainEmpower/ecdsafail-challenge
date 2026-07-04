# `proof_toolkit` — design notes

Why the replayer is shaped the way it is. Usage lives in [`README.md`](README.md); the
scoping decisions live in ADRs [0028](../../adr/0028-reusable-proof-toolkit.md) and
[0029](../../adr/0029-proof-toolkit-op-stream-replayer.md). This file is the rationale a
future maintainer needs before changing `symsim.py`.

## 1. The problem: prove what you *emit*, not a re-implementation

A proof is only as good as the thing it is bound to. The repo's other formal layers
prove *re-implementations* of the circuit:

- `solinas_reduction.py` mirrors the algorithm "step-for-step" in z3 BitVec — a faithful
  **model**.
- `kani_proofs.rs::solinas_add` is a hand-written integer **twin** of the control flow.

Both can pass while the gate-emitting builder drifts away from them — exactly the
copy↔emitter gap referee finding **F2** names. The toolkit exists to close that gap: it
takes the op-stream the real `B` builder emits (dumped to JSON by a `#[cfg(test)]`
harness whose drift guard keeps the artifact byte-identical to a fresh emit) and proves
the claim over **those gates**. The proof then verifies *what you run*.

This is the transferable asset of the repo — the *methodology*, not the score-specialized
gates. ADR 0028 is explicit that the hand-tuned `src/point_add/` primitives are **not**
carved into a library (byte-identical `ops.bin` risk, single consumer, curve-specific);
the reusable thing is this how-to-verify pattern.

## 2. Core idea: symbolic replay of the *simulator's* semantics

`SymSim` mirrors `Simulator::apply_iter` (`src/sim.rs`) **op-for-op**. It is a symbolic
interpreter of the same per-op state transitions the scorer runs — CX/CCX toggles, the
`HMR` measurement + phase kickback, the `cz_if` conditional phase, the condition stack —
but over z3 `Bool`s instead of `u64` lanes. Read `symsim.py::SymSim.apply` next to
`sim.rs::apply_iter`: each branch is a line-for-line translation. That parallelism is the
point — it is what makes "the proof runs the emitted gates" true.

## 3. The modelling decisions

**One symbolic shot models all 64.** The real simulator packs 64 independent shots into
each `u64` (qubit lane, bit lane, phase). Every op is bitwise-parallel across lanes — no
op mixes lanes. So a single symbolic shot (one `Bool` per qubit/bit, one `Bool` phase)
is an exact model of one lane, and a claim proved for it holds for every shot. This is
why the model is `Bool`, not `BitVec[64]`.

**Measurement outcomes are free (∀).** In `sim.rs`, `HMR`/`R` read a random `u64` from an
XOF; the qubit resets to `|0>` and the phase picks up `q · rng`. The toolkit instead
mints a **fresh free `Bool`** per measurement (collected in `SymState.meas`). A property
proved this way holds for *every* measurement outcome, not the sampled XOF draws — the
guarantee the 9024-shot sample could not give. `HMR` also records the outcome into a
classical bit (so a later `cz_if` can read it); `R` is the same minus the record.

**Condition handling matches `apply_iter` exactly.** `current_base_condition` (all-ones ⇒
`True`) and its push/pop stack are modelled; every op's effective condition is
`base ∧ bit(c_condition)`. This is what makes classically-conditioned gates (`cz_if`,
`x_if`, `PUSH/POP_CONDITION`) faithful rather than approximated.

**Lazy `|0>` / `False` defaults.** Only the free-input registers are seeded; every other
qubit/bit is implicitly `|0>`/`False` until written. Ancilla cleanliness is then just
"this id is back to `False` at the end".

## 4. What this does and does *not* cover

It covers the gap between **the emitted gates** and **the proof**. It does **not** cover
the gap between **`sim.rs`** and **`symsim.py`** — the model is still hand-written Python,
so a bug that mis-models an op could mask a real defect. Three things bound that residual
risk:

1. `selftest.py` pins each op's symbolic semantics against hand-computed truth,
   independent of any dumped artifact — a regression in the op model fails there.
2. The Rust drift guard binds the committed JSON to a fresh emit, so the *stream* is
   never stale.
3. The **Kani** harness ([ADR 0030](../../adr/0030-kani-harness-bound-to-emitter.md))
   drives the *actual* `Simulator` (not this model) for the same adder. z3-via-model and
   Kani-via-real-simulator are independent bindings: a model bug in one does not mask a
   bug the other would catch.

## 5. Teeth: proving a correction is load-bearing

A proof that something is clean is stronger when you can show the cleaning step is
*necessary*. `drop_phase_kinds` (⊆ `{CZ, Z, CCZ, NEG}`) suppresses the phase
contribution of the named phase ops; re-running the proof with, say, the `cz_if` CZ
dropped must make the phase claim **fail** (`find(...) == sat`). ADR 0027's teeth: delete
the `cz_if` fixups ⇒ the HMR kickback survives ⇒ some input+outcome has net phase 1. Kept
general so any op-stream proof can demonstrate its own load-bearing corrections.

## 6. Keep the reference independent

The replayed output is the *implementation*. The claim must compare it against a
**reference computed a different way**, or the proof is circular. `ripple_carry_sum` is a
textbook adder reference (not the emitted Cuccaro sweep); `solinas_reduction_emitted.py`
checks the Solinas `+c`/overflow gates against a ripple-carry-add + conditional-
subtract-p reduction — structurally different from what it replays. When adding a proof,
resist reusing the implementation's own structure as the spec.

## 7. Tractability

Everything is pure `Bool` (`And`/`Or`/`Xor`/`Not`); z3 bit-blasts and refutes the negated
conjunction. Cost scales with op-count × output-width × free-var count. Observed:

- `mbuc_phase_correction.py` — `cuccaro_add_fast`, widths 2..**256**, ~1.5k ops, 510 free
  outcomes at 256: **~2 s** total.
- `solinas_reduction_emitted.py` — `mod_add_qq`, 256-bit, ~7.2k ops, 521 free outcomes,
  `mod p` reference: **~4.7 min**.

The reduction proof is ~140× the adder proof, so it is a **standalone** recipe kept out of
the default `just analysis` suite (like `just kani`). Rule of thumb: prove at the smallest
width that is faithful; only go to 256 when the width itself is the point (as it is for a
production-width binding). BMC (Kani) does not scale to 256 at all — that width is z3's
job; small-width real-type binding is Kani's.

## 8. Extending the model

Adding an op kind means adding a branch to `SymSim.apply` that mirrors the corresponding
`sim.rs::apply_iter` arm, adding it to `selftest.py` with a hand-checked case, and (if it
carries phase) to `PHASE_OP_KINDS`. Keep the op-name string identical to the Rust
emitter's `op_name`/`from_name` (canonical **uppercase**, e.g. `"SWAP"` — a mismatch there
was a real bug caught in review). Do not add semantics the simulator doesn't have; the
whole value is faithfulness to `sim.rs`.

## 9. Deliberate non-goals

- **Not the scored primitives.** No Cuccaro/Kaliski/Solinas *gates* live here; those stay
  in `src/point_add/`, byte-identical (ADR 0028). The toolkit only *reads* their emitted
  output. **Scope caveat ([ADR 0035](../../adr/0035-proof-scope-scored-vs-reference.md)):**
  the primitives the current consumers dump (`cuccaro_add_fast`, `mod_*_qq_fast`) are the
  *reusable reference* arithmetic in `arith/modular/`; the **scored** `ops.bin` is built by
  `trailmix_ludicrous` and does not emit them. The replay proofs bind those reference
  primitives; redirecting the toolkit at `trailmix_ludicrous`'s own adder/comparator (to
  bind *scored* gates) is a tractable, not-yet-done next step.
- **Not a general quantum simulator.** It models exactly `sim.rs`'s stabilizer-ish
  per-op semantics (X/CX/CCX/phase/measurement over one classical shot), which is all the
  scored circuit uses — not arbitrary amplitudes.
- **Not a clean-room primitive crate.** ADR 0028 defers that to a separate effort with an
  explicit trigger (a second consumer of the *primitives*); the toolkit is the
  *methodology*, whose second consumer (the reduction proof, ADR 0031) already exists.
