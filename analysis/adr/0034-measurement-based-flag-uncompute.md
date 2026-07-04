# ADR 0034 — Evaluate the measurement-based flag uncompute for `mod_*_qq_fast` (score lead #1)

**Status:** Rejected (measured no-op) — the `MOD_FAST_FLAG_CONDITIONAL_REPLAY` lever
produces a **byte-identical `ops.bin`**: the scored circuit is built by
`trailmix_ludicrous`, which does not use `mod_*_qq_fast` at all, so the env var is dead on
the scored path. Recorded for the negative result and the reframing it surfaced (the
scored reduction-flag Toffoli lives in `trailmix_ludicrous`, not the proven
`arith/modular/*_fast` primitives). See `ROADMAP.md` → Score-optimization leads,
issue #77.
**Date:** 2026-07-04

## Context

**Hypothesis going in (from ADR 0027/0032) — overturned by the measurement below.** The
scored hot path was taken to run 58 `mod_*_qq_fast` calls. On that assumption, each does a
Solinas reduction whose correctness `flag` (reduction-needed) must be uncomputed to keep the
ancilla clean. The emitted `_fast` adder's carry uncompute is already measurement-based
(HMR + `cz_if`, **0 Toffoli**, proven ADR 0027), so the Toffoli that remains in a
`mod_*_qq_fast` is concentrated in the **flag uncompute** — the `cmp_lt`/`neg` machinery
around the reduction (the dumps in ADR 0032 show `sub` costs more Toffoli than `add`
precisely because of its extra neg+compare).

`mod_add_qq_fast` / `mod_sub_qq_fast` contain **two** flag-uncompute paths:

- **default** — `cmp_lt_into_fast(acc, a, flag)`: an *unconditional* borrow-chain compare
  that XORs the result into `flag`. Its CCX execute on **every** shot.
- **`MOD_FAST_FLAG_CONDITIONAL_REPLAY=1`** — `hmr(flag)` measures the flag out to a
  classical bit, then `cmp_lt_phase_conditioned(acc, a, measured)` re-derives the same
  relation *under a `push_condition(measured)`*. Its CCX are classically conditioned on the
  measured flag.

The score's Toffoli factor is the **dynamic, condition-masked** average
(`sim.rs` counts `cond.count_ones()` per Toffoli), not a static op count. For roughly
uniform field inputs the reduction fires on ~half the shots, so a flag-conditioned compare
could execute its Toffoli on only that fraction — potentially lowering avg-Toffoli. This is
a hypothesis to **measure**, not assume (the default is already measurement-based, so the
static gate counts are similar; the difference, if any, is dynamic).

## Decision (proposed)

1. **Measure** the full score both ways: `cargo run --bin build_circuit` (default) vs
   `MOD_FAST_FLAG_CONDITIONAL_REPLAY=1 … build_circuit`, each scored by the trusted
   `eval_circuit` (`score = round(avg_toffoli) × qubits`). Compare `avg_toffoli`, `qubits`,
   and `score`, and confirm `eval_circuit` still reports correct/reversible/phase-clean.
2. **If** the conditional-replay path lowers the score **and** stays correct: prove the
   emitted conditional-replay `mod_*_qq_fast` phase-clean + functionally correct with the
   `proof_toolkit` (extend `mod_fast_reduction_emitted.py` to the new op-stream — the
   `push_condition`/`cz`/`hmr` ops are already modelled), then flip the default and
   re-score. Adopting changes `ops.bin`'s SHA — a deliberate score change (issue #6's
   editable path), gated on the proof.
3. **If** it does not lower the score: record the negative result here and move to
   lead #2 / #3. The investigation is the deliverable either way.

## Measurement

Built + scored (trusted `eval_circuit`) both ways:

| build | avg_toffoli | qubits | score | `ops.bin` SHA-256[..16] |
|---|---|---|---|---|
| default | 1,364,230 | 1,152 | 1,571,592,960 | `f30d8365c1235002` |
| `MOD_FAST_FLAG_CONDITIONAL_REPLAY=1` | 1,364,230 | 1,152 | 1,571,592,960 | `f30d8365c1235002` |

**Identical — byte-for-byte.** The env var has **zero** effect on the scored circuit.

Root cause: `point_add::build()` → `trailmix_ludicrous::build_trailmix_ludicrous_ops()`,
and `grep` confirms `trailmix_ludicrous/` references neither `MOD_FAST_FLAG_CONDITIONAL_REPLAY`
nor `mod_add_qq_fast`/`cuccaro_add_fast`/`add_nbit_qq`. The scored circuit's modular
arithmetic is `trailmix_ludicrous/{arith,gidney,comparator,square,gcd}.rs` — its own adder
(`hybrid_add_adaptive`), comparator (`compare_geq_cin_middle`), and Kaliski inverse — a
separate, more-optimized implementation from the `arith/modular/*_fast` primitives.

## The reframing this surfaces

The proven primitives (`cuccaro_add_fast`, `mod_add_qq`, `mod_*_qq_fast` — ADR
0027/0030/0031/0032) are **not** the gates the scored `ops.bin` runs; they are the
`arith/modular/*_fast` family, exercised by tests/other paths, not by
`trailmix_ludicrous`. So:

- **Lead #1 dies here.** The measurement-based flag uncompute cannot help the score
  because its primitive is not on the scored path.
- **The real lead-#1 target moves to `trailmix_ludicrous`**: the scored reduction/compare
  Toffoli lives in `comparator::compare_geq_cin_middle` and the `arith.rs` reduction
  machinery. Any flag/reduction optimization must be applied there — tracked back in
  `ROADMAP.md` (lead #1, retargeted) and issue #77.
- **Separately, an honest-scope item** (not a score lead): the emitter-bound proof arc
  binds real, reusable primitives but not the *scored trailmix gates*. Whether to redirect
  the `proof_toolkit` at the `trailmix_ludicrous` op-stream is a distinct decision
  (recorded as a follow-up consideration, not resolved here).

## Consequences

- **The proof toolkit did its job cheaply — as a fast falsifier.** One build+score pair
  (no code change) killed a plausible-looking lead and redirected effort, which is exactly
  the point of lead #4 (verification-informed tuning): fail fast, before writing a rewrite.
- **No scored-circuit change.** Measurement only (env var + existing binaries); `ops.bin`
  is byte-identical (`f30d8365c1235002`).
- **Isolation ([ADR 0001](0001-analysis-layer-isolated-from-score.md)).** Does not edit
  `src/point_add/`.
