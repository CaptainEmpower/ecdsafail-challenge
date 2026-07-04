# ADR 0035 — Proof scope: reference primitives vs the scored circuit

**Status:** Accepted (disclosure) — states precisely which parts of the emitter-bound
proof arc bind the **scored** `ops.bin` and which bind **reusable reference primitives**
that the scored circuit does not emit. No code change; sharpens the honest-scope framing
(ADR 0026) after the ADR 0034 measurement surfaced the distinction.
**Date:** 2026-07-04

## Context

The scored circuit is built entirely by `trailmix_ludicrous::build_trailmix_ludicrous_ops()`
(`point_add::build()`), a hand-tuned arithmetic implementation
(`trailmix_ludicrous/{arith,gidney,comparator,square,gcd}.rs` — its own
`hybrid_add_adaptive` adder, `compare_geq_cin_middle`, and Kaliski inverse).

The emitter-bound proofs (ADR 0027/0030/0031/0032) dump and verify the
`arith/modular/*_fast` family — `cuccaro_add_fast`, `mod_add_qq`, `mod_*_qq_fast`. ADR
0034's measurement established that **`build()` does not emit any of these**: they are a
separate, self-consistent *reference / clean-room-in-waiting* arithmetic layer (marked
`#[allow(dead_code)] // retained reference/alternative impl`), used by the proof harnesses
and the `arith/multiply` tree, not by the scored path. So those proofs bind **real,
reusable primitives — but not the gates `ops.bin` runs.**

This does not retract any proof; every ADR 0027–0032 claim remains true of the primitive
it names. It sharpens *what the primitive is*.

## Decision

Disclose the split explicitly, everywhere the proofs are described.

**Bound to the SCORED circuit (`ops.bin`):**
- **Constprop soundness — ADR 0033.** `constprop::run(ops, reg0+reg1)`
  (`trailmix_ludicrous/mod.rs:465`) is applied to the *scored* op-stream; the affine-form
  soundness premise is proved at the 512-variable production universe that call uses. This
  is a proof about the scored circuit's optimization stage.
- **End-to-end sampled validation.** `eval_circuit` simulates the actual scored gates for
  correctness / reversibility / phase over the 9024-shot Fiat–Shamir sample (reproduced by
  the referee, ADR 0023). Sampled, not exhaustive — the standing §-honesty limitation.

**Bound to REFERENCE primitives (not `ops.bin`):**
- The emitter-bound arithmetic proofs — ADR 0027 (`cuccaro_add_fast`), ADR 0030 (Kani, same
  adder), ADR 0031 (`mod_add_qq`), ADR 0032 (`mod_*_qq_fast`) — and ADR 0024's Kani twin.
  These prove reusable primitives + the `proof_toolkit` methodology; they are *not* the
  scored `trailmix_ludicrous` gates.

## Can the scored circuit itself be proved?

Partially. Honest breakdown by tractability:

- **Its arithmetic *primitives* — yes, achievable.** Dump the emitted op-streams of
  `trailmix_ludicrous`'s own `hybrid_add_adaptive` (adder) and `compare_geq_cin_middle`
  (comparator) and replay them through `proof_toolkit`, exactly as done for
  `cuccaro_add_fast`. These are adder/comparator-sized (~10³–10⁴ ops), so z3-tractable at
  256-bit. This would upgrade "we proved a reference sibling" to "we proved the scored
  circuit's core arithmetic primitives" — the natural next increment (tracked as a lead).
- **Its Kaliski modular inverse (`gcd.rs`) and squaring at 256 — no.** Data-dependent
  iteration and O(n²) structure put these past z3/BMC at production width (only toy-scale is
  proved, ADR 0020/0021). Same wall as the composed point-add.
- **The composed ~10M-op point-add end-to-end — no.** Intractable in either solver; stays
  guarded by the 9024-shot sample (the standing stretch, `scientific-value.md` §4).

So "prove the scored circuit" resolves to: the optimizer stage is proved (0033); the
tractable scored primitives (adder, comparator) *can* be proved emitter-bound and aren't
yet; the inverse / squaring / full composition are intractable and remain sampled.

## Consequences

- **Honest, precise scope.** A reader now knows the emitter-bound arithmetic proofs cover
  reusable reference primitives, while what binds the scored circuit is the constprop
  soundness proof (0033) plus the sampled end-to-end check — no overclaim that "the scored
  circuit is machine-checked".
- **Names a concrete next step.** Redirecting `proof_toolkit` at `trailmix_ludicrous`'s
  adder + comparator is a tractable way to bind proofs to *scored* gates; recorded as a
  score/rigor lead rather than done here.
- **No retraction.** ADR 0027–0032 stand; this ADR only classifies their subject.
- **Consistent with ADR 0028.** The reference `arith/` layer is the "reusable primitives /
  clean-room-in-waiting" ADR 0028 described; it is retained deliberately, and its proofs
  are the methodology demonstration — not a claim about the scored artifact.
