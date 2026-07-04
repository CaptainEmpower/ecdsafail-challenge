# ADR 0032 — Prove the scored `_fast` modular wrappers over the emitted gates (referee F2)

**Status:** Accepted — implemented in `analysis/verify/mod_fast_reduction_emitted.py`
(z3, via `proof_toolkit`) over the emitted op-streams dumped by
`src/point_add/modfast_dump.rs` (`#[cfg(test)]`) into `analysis/mod_fast_ops.json`, wired
as `just mod-fast-emitted` and run by the nightly `heavy-proofs` workflow. All three
wrappers (`add`, `sub`, `double`) are proved `unsat` on the negated claims (timings under
*As built*). Extends ADR 0031 from the plain `mod_add_qq` to the `_fast` wrappers the
**scored hot path** actually runs. Analysis-layer only; the scored circuit is
byte-identical (`ops.bin` SHA `f30d8365…`).
**Date:** 2026-07-04

## Context

The scored circuit runs its modular arithmetic almost entirely through the `_fast`
wrappers — `mod_add_qq_fast` / `mod_sub_qq_fast` / `mod_double_inplace_fast`, the **58
fast calls vs 3 plain `mod_add_qq`** noted in ADR 0027. The prior emitter-bound proofs
covered the shared `cuccaro_add_fast` *adder* (ADR 0027 z3, ADR 0030 Kani) and the plain
`mod_add_qq` *reduction* (ADR 0031 z3). The `_fast` wrappers — which fold the Solinas
reduction around the **measurement-based** adder — were still covered only by the sampled
end-to-end check, not by a proof over their emitted gates.

Unlike the plain `mod_add_qq` (whose only phase-bearing ops are `R` resets), the `_fast`
streams carry `HMR` + `cz_if` (`CZ`) throughout — so a proof of them exercises the
measurement-based uncompute **in the full reduction context**, not just the adder in
isolation.

## Decision

Apply the ADR 0031 "prove-what-you-emit" pattern to all three `_fast` wrappers.

1. **Dump the real op-streams** (`src/point_add/modfast_dump.rs`, `#[cfg(test)]`) at the
   **default builder configuration** — no `SECP_DIRECT_CONST_ARITH` / `KAL_VENT_*` /
   `MOD_FAST_*` env vars, i.e. exactly what `build_circuit` uses — so the proof covers the
   scored gates. Sizes: add ≈ 11.3k ops / 1024 HMR, sub ≈ 15.6k / 1277 HMR, double ≈ 3.1k
   / 255 HMR + 256 `SWAP`. A drift-guard test keeps the committed artifact byte-identical.

2. **Replay + prove** (`analysis/verify/mod_fast_reduction_emitted.py`) through the
   `proof_toolkit` z3 model, HMR/`R` outcomes free/∀, over all field-element inputs:
   - **add**: `acc' == (acc + a) mod p`, `a` preserved — canonical, fully reduced.
   - **sub**: `acc' == (acc - a) mod p`, `a` preserved — canonical, fully reduced.
   - **double**: `v' ≡ 2·v (mod p)` with `v' < 2^n` (see below).
   - all: flag + every ancilla → |0>, **net phase 0 for every measurement outcome**.
   References are the independent `proof_toolkit.refspec` arithmetic (ripple-carry +
   conditional sub/add), structurally unlike the replayed Solinas `+c`/overflow path.

3. **`refspec` in the toolkit.** The reference `x±y mod p` / `2x mod p` primitives are
   added to `proof_toolkit` (`const_bits`, `add_bits`, `sub_bits`, `ult`, `bits_eq`,
   `mod_add`, `mod_sub`, `mod_double_canonical`) — reusable independent-reference
   arithmetic for any future emitted-gate proof.

## A finding: `mod_double_inplace_fast` is a *lazy* reduction

The symbolic proof surfaces something the sampled test cannot. `mod_double_inplace_fast`
performs a **single** conditional fold (add `c` on the `2^n` carry-out). For
`v ∈ [2^255 − c/2, 2^255)` — a ~2³¹-wide window — the doubled value lands in `[p, 2^n)`
and is left **unreduced**: `v'` is congruent to `2v (mod p)` and `< 2^n`, but is `2v mod p`
**or** `2v mod p + p`, not always the canonical representative. Its 64-shot unit test
asserts full reduction yet never samples that window (probability ≈ 2⁻²²⁵), so a strict
`v' == (2v) mod p` claim would (correctly) fail. The proof therefore states the contract
the circuit actually satisfies — congruence mod p, `v' < 2^n` — which is sound because all
downstream consumers are themselves mod-p. This is exactly the sampling-blind-spot a
symbolic proof is for; it is disclosed, not papered over.

## As built

`just mod-fast-emitted` (or `… mod-fast-emitted add|sub|double` to run one). Each claim
group (functional / a-preserved / ancilla-clean / phase) is discharged in its **own** z3
solve — the phase clause over ~10³ free HMR outcomes then never has to interact with the
functional clause. All groups return `unsat` on their negation. Timings (single core):

| op | functional | preserved | clean | phase (outcomes) |
|----|-----------|-----------|-------|------------------|
| `add`    | 161.5 s | 16.8 s | 0.0 s | 32.2 s (∀ 2569) |
| `sub`    | 92.1 s  | 38.3 s | 0.1 s | 40.3 s (∀ 3330) |
| `double` | 1.3 s (congruence) | — | 0.0 s | 0.2 s (∀ 768) |

≈ 6.5 min end-to-end. Two things made it tractable (both reusable): (1) `proof_toolkit`
emits `SWAP` as the closed-form exchange `(a,b) := If(cond,(b,a),(a,b))` instead of the
nested 3-XOR sweep — algebraically identical to `sim.rs`, but `double`'s 256-swap rotation
stays a permutation of inputs rather than a depth-256 XOR tree, taking `double` from
13+ min (never finishing) to ~1 s; (2) `double`'s functional claim is stated as
`2·v − v' ∈ {0, p}` (a shallow shift minus the output, compared against two *constants*)
rather than an equality between two nested modular reductions.

Kept **out** of the default `just analysis` (like `solinas-emitted` / `kani`); run
explicitly, and continuously by the nightly `heavy-proofs` GitHub workflow. The Rust drift
guard binding the artifact to the emitter runs in the normal `cargo test` job.

## Consequences

- **F2 closed on the scored hot path, not just the plain variant.** The modular
  arithmetic the circuit actually runs (the `_fast` wrappers) is now proved correct and
  phase-clean over its emitted gates — the measurement-based Solinas fold verified in
  context, over all inputs and all measurement outcomes.
- **A real lazy-reduction disclosed.** `mod_double_inplace_fast` does not fully reduce on
  a narrow input window; harmless downstream (all mod-p) but now stated precisely rather
  than assumed from a sampling test that never hits it.
- **Third `proof_toolkit` consumer + `refspec`.** Reinforces ADR 0028/0029: a new family
  of primitives covered with a dump harness, a claim script, and reusable reference
  arithmetic — no re-derivation of sim semantics.
- **Honest scope.** Per-primitive proofs, not the whole composed point-add (still the
  standing z3-intractable stretch, `scientific-value.md` §4). Single width 256 (the
  wrappers bake `c = 2^256 − p`).
- **Isolation ([ADR 0001](0001-analysis-layer-isolated-from-score.md)).** A `verify/`
  script + `#[cfg(test)]` dump/guard; never compiled into `build_circuit`. Scored circuit
  byte-identical (`ops.bin` SHA `f30d8365c1235002`).
