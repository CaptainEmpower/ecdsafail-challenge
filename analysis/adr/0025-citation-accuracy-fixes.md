# ADR 0025 — Citation & metric-wording accuracy fixes (referee F6/F7/F8, issue #61)

**Status:** Accepted — documentation-only corrections.
Follows [ADR 0023](0023-external-referee-review.md) (findings F6/F7/F8).
**Date:** 2026-07-03

## Context

The referee review found three accuracy defects in the written comparisons.
None affects a number the code produces; all are about how prior work and the
repo's own metric are described.

- **F6 — Chevignard misattribution.** "Chevignard 1098 q — the current qubit
  frontier" was applied to secp256k1. Per the primary work, **1098 is the P-224
  figure; the P-256 figure is 1193.** Verified against two independent secondary
  sources during review.
- **F7 — "published" vs "private" point-addition bounds.** `README.md` labels the
  `2.7M/1175` and `2.1M/1425` figures as Google's *private* Pareto points, while
  `technical-report.md` and `ecdlp_estimate.py` called them the paper's
  *published, zero-knowledge-proven* PA bounds. Babbush et al.'s public headline
  is the full-ECDLP totals; per-point-addition sub-bounds could not be confirmed
  in the public abstract during review, and even `README.md` itself mixed
  "private" (table) with "published" (prose).
- **F8 — "peak qubits".** `scientific-value.md` called the README's "peak qubits"
  label *inaccurate* (`qubits = max_id+1`). In fact register reuse makes
  `max_id+1 = 1152 =` true peak (the `ladder_composition` Δ=0 result), so the
  README is correct and the caveat was over-cautious.

## Decision

- **F6:** state "1193 q for P-256 (1098 is P-224)" in `novelty-assessment.md`
  (table + "Not the qubit-count frontier"), `scientific-value.md`,
  `technical-report.md` (§5 + §6), and `outline.md`. The corrected P-256
  frontier (1193) sits *above* the A2 bound this repo cites (1168) — i.e. the A2
  bound is below the frontier — so resting "not the frontier" on the A2 bound
  would be misleading (the circuit does not independently realize 1168). The claim
  is instead re-anchored on the repo's own faithful quantum-addend port
  (1424–1680), which is above the 1193 frontier.
- **F7:** describe the PA Pareto points as "Babbush et al. 2026 point-addition
  operating points (the challenge's reference numbers)", state that the public
  headline is the full-ECDLP totals, and add a pre-submission action to pin the
  exact source. Reconcile `README.md`'s internal "private"/"published" mix.
- **F8:** rewrite the `scientific-value.md` limitation to say `max_id+1` *equals*
  peak simultaneous width here (ids are recycled) and is a conservative proxy
  (over-counts, never under-counts) — so the README label is accurate.

## Consequences

- The literature comparison and the repo's own metric description are now
  internally consistent and match the primary sources.
- One pre-submission action remains (F7): pin the exact provenance of the PA
  Pareto points to a paper table or an organizer-supplied number.
- Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md):
  documentation only; the scored circuit is byte-identical.
