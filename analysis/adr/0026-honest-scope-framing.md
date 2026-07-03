# ADR 0026 — Honest-scope framing pullback (referee F1/F2/F4/F5, issue #57/#59/#60)

**Status:** Accepted — documentation-only framing corrections.
Follows [ADR 0023](0023-external-referee-review.md) (findings F1/F2/F4/F5).
**Date:** 2026-07-03

## Context

The referee review found that four load-bearing verbs in the written claims —
"machine-checked", "bound to implementation types", "exactly computed",
"demonstrated end-to-end" — describe *models* of the artifact more than the
scored artifact itself. The buried caveats were already correct; the
abstract/contribution sentences overreached. None of the underlying code is
wrong; the claims about it were too strong.

- **F1.** z3/Kani model the **plain** `mod_add_qq` and treat adders as exact
  integer `+`/`−`. The scored hot path emits `mod_add_qq_fast` (58 call sites vs
  3 for the plain variant) — a `cuccaro_add_fast` with measurement-based uncompute
  (`hmr` + `cz_if` phase-kickback) whose phase logic has no symbolic coverage.
- **F2.** `kani_proofs.rs` verifies a hand-written integer re-implementation
  ("on plain integers instead of emitted gates"), not the gate-emitting builder.
- **F4.** `shor_ecdlp_recovery.py`'s oracle is a Python model adder (chord-only,
  `inv(0):=0`, toy field), deterministic and phase-clean; the scored circuit
  measures probabilistic phase garbage on `dx=0` (`completeness_probe.rs`). The
  scored circuit is never run through the recovery.
- **F5.** The "exact end-to-end bound" is exact only for `n = 1009/2003`; at
  attack scale it is the analytic union bound `28·2/n`, i.e. the equidistribution
  value — so `completeness_argument.md`'s "equidistribution no longer load-bearing"
  was inaccurate for the headline. The `≈2⁻²⁵⁰` figure also presumes the offset
  encoding, which the scored single-point-add `build()` does not implement.

## Decision

Re-scope the prose, without weakening any true statement, across the citable and
analysis docs:

- **Abstract / Contributions (`technical-report.md`):** "machine-checked" →
  "machine-checked **integer core**"; name the plain-vs-`_fast` gap and the Kani
  re-implementation explicitly; "computed + circuit-verified" → "computed +
  circuit-**confirmed** (exact at toy scale, analytic union bound at attack
  scale)"; "demonstrated end-to-end ... using the incomplete adder this circuit
  implements" → "demonstrated **at toy scale** with a **model** of the incomplete
  adder"; make the amplitude bound (not the recovery) the load-bearing result.
- **`scientific-value.md`:** add scope boxes to §0 and §1c stating the same
  boundaries.
- **`completeness_argument.md`:** correct the equidistribution caveat (F5) and add
  a model-adder / phase-garbage caveat to the recovery bullet (F4).
- **`novelty-assessment.md`:** add a scope-discipline box after the differentiating
  items.

This ADR is disclosure, not new proof. What *is* closed by construction is F3
(production-width z3 proof, ADR 0024, PR #64); F1/F2/F4/F5 are closed by making the prose
match the artifact. An optional future increment (a z3/sim model of the `hmr` +
`cz_if` phase correction, or a Kani harness bound to the emitter) would upgrade
F1/F2 from disclosure to coverage.

## Consequences

- The written claims now match what the code establishes: a machine-checked
  **integer core**, a **simulation-backed** completeness argument, and a toy-scale
  **model** recovery — still a genuine delta versus fuzz-only correctness and
  negligibility-by-citation, but no longer overstated.
- The real, reproducible headline (`1,364,230 Toffoli × 1,152 qubits`, 9024/9024
  shots) and the z3 integer-level Solinas proof are untouched.
- Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md):
  documentation only; the scored circuit is byte-identical.
