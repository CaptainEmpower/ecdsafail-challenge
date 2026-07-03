# ADR 0024 — Prove the adder/comparator recurrences at production 256/257-bit width (referee F3, issue #58)

**Status:** Accepted — implemented in `analysis/verify/peephole_identities.py`.
Follows [ADR 0023](0023-external-referee-review.md) (finding F3).
**Date:** 2026-07-03

## Context

The referee review (F3, `paper/REVIEW.md`) noted that
`peephole_identities.py` proved the ripple-carry recurrence and the borrow-chain
comparator only for `w ∈ {1,2,3,4,8,16,32,64}`. These are concrete bit-blasted
widths, **not** an induction on `n`, so "the 256-bit adder is correct" was
*extrapolated* from ≤64-bit instances rather than proved at the width the scored
circuit actually runs (256-bit coordinate registers; the 257-bit Solinas
extended register).

## Decision

Add the production widths `256` and `257` to both width loops. z3 discharges
each instance by bit-blasting the ripple/borrow recurrence; measured cost is
<0.2 s per instance (0.64 s for the whole suite), so there is no reason to stop
at 64.

The lemma count rises from `22/22` to `26/26`; `scientific-value.md §1b` is
updated to match (`w∈{1..64, 256, 257}`).

This does **not** address F1/F2 — the proofs still model the *plain*
`mod_add_qq`/comparator recurrence, not the emitted `_fast` measurement-based
variant with HMR + `cz_if` phase correction. That scope boundary is disclosed
separately (ADR 0026). What F3/0024 fixes is narrow and exact: the recurrence the
z3 layer *does* model is now proved at production width, closing the
"extrapolated to 256" gap for that layer.

## Consequences

- The ripple-carry and comparator recurrences are proved at 256/257-bit — the
  actual register widths — removing the width-extrapolation caveat for the
  modelled (plain) adder.
- Cheap and deterministic: +4 lemmas, +~0.5 s of z3, no new dependency.
- Consistent with [ADR 0001](0001-analysis-layer-isolated-from-score.md):
  analysis layer only; the scored circuit is byte-identical (`ops.bin`
  unchanged).
