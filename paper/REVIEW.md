# Independent referee review — codebase & written claims

*Status: external review, 2026-07-03. Reviewer perspective: cryptography /
resource-estimation. This document records an independent, skeptical read of the
repository and its Markdown statements, with every claim checked against the
actual artifact (rebuild + re-score + re-run of the analysis suite) rather than
the prose. It is the input to the remediation tracked in
[ADR 0023](../analysis/adr/0023-external-referee-review.md) and the issues it
links.*

## Method

- Rebuilt the circuit and re-ran the scorer over the full 9024-shot harness.
- Re-ran the z3 suite, the 13-stage Python analysis suite, `depth_report`,
  `ecdlp_estimate.py`, and `cost_model.py`.
- Verified the three external citations (Babbush arXiv:2603.28846; Han Luo
  arXiv:2604.02311; Chevignard ePrint 2026/280) against primary/secondary
  sources.
- Audited the completeness and formal-proof claims against the *scored* code
  path (`build()` → emitted op stream), not only the analysis-layer models.

## Bottom line

This is a serious, unusually well-disciplined artifact, and its central
competitive claim is **real and reproducible**. An independent rebuild scores
the circuit at **1,364,230 average Toffoli × 1,152 qubits** with **9024/9024
shots passing** classical correctness, reversibility, ancilla-cleanliness, and
phase checks — genuinely under Babbush et al.'s Low-Qubit point-addition
operating point on both axes. The z3 proofs, Kani harnesses, and analysis suite
all exist and run clean, and the ADR trail is honest and self-critical.

The gap this review documents is not between the artifact and reality — it is
between the **artifact** (strong, honest, reproducible) and the **top-line
framing** (which repeatedly promotes toy-scale, model-level, or width-limited
results to artifact-level, all-inputs claims). In almost every case the buried
caveat is already correct; the abstract/contribution sentence is what overreaches.
The ECDLP's hardness is untouched and the docs are correct that there is no break.

## What holds up (verified)

- **The score.** Reproduced exactly; `score.json` → `1,571,592,960`. The harness
  (`sim.rs`, `circuit.rs`) is the unmodified, Trail-of-Bits-hardened upstream, so
  the scoring cannot be trivially gamed. A verification run independently
  reproduced the committed `results.tsv` row (`1364229.770 / 1152`).
- **z3 integer-level Solinas proof.** `analysis/verify/solinas_reduction.py`
  genuinely proves `mod_add_qq: low256 == (acc+a) mod p` and the overflow-ancilla
  uncompute, over all `acc,a ∈ [0,p)`, against the real secp256k1
  `p = 2²⁵⁶−2³²−977`. Faithful step-for-step to `arith/modular/add.rs:9-46`.
- **All three external citations are real,** and `validate_reference_adders.py`
  genuinely rejects the source paper's negative-control circuits.
- **The numeric pipeline is internally consistent** and reproduces, including the
  `measured_mbuc == (PA+3·2¹⁶)·28 − 6·28` cross-check.

## Findings (ranked by materiality)

Each finding maps to a remediation issue; see
[ADR 0023](../analysis/adr/0023-external-referee-review.md).

### F1 — z3/Kani cover the *unused* arithmetic variant; the scored circuit runs a different, unmodeled one
The hot-path modular multiply emits `mod_add_qq_fast` / `mod_sub_qq_fast` /
`mod_double_inplace_fast` (58 fast calls vs 3 plain `mod_add_qq` in
`src/point_add/arith/`), which route through `cuccaro_add_fast` with
**measurement-based uncompute** (`b.hmr(...)` + `b.cz_if(...)` phase-kickback
correction). z3 and Kani model only the plain `mod_add_qq` and treat the adders
as exact integer `+`/`−`; the HMR randomization and CZ phase logic the emitted
circuit depends on have **zero symbolic coverage**. "Machine-checked arithmetic,
bound to the implementation types" is therefore true only for a primitive that
appears 3 times and not in the emitted form. **Severity: high** (scope framing).

### F2 — Kani proves a hand-written copy, not the production code path
`src/kani_proofs.rs` defines standalone `solinas_add` / `solinas_add_small`
integer functions and verifies those — the file itself says "on plain integers
instead of emitted gates." It never invokes the gate-emitting builder; if the
copy and the emitter drift, Kani stays green. The `_u256` harness does use the
real `U256` / `SECP256K1_P` (good), but its oracle is the same algebraic shape as
the function under test. **Severity: high** (scope framing).

### F3 — Adder/comparator lemmas proved at ≤64-bit and extrapolated to 256
`analysis/verify/peephole_identities.py` proves the ripple recurrence and the
borrow-chain comparator only for `w ∈ {1,2,3,4,8,16,32,64}` — concrete
expansions, not induction on `n`. The production registers are 256/257-bit. z3
solves the 256/257-bit instances in <0.2 s each (verified), so this is a directly
closable gap, not a fundamental limit. **Severity: medium; directly fixable.**

### F4 — The "demonstrated end-to-end attack" runs a benign stand-in adder, not the scored circuit
`analysis/verify/shor_ecdlp_recovery.py` is a genuine ideal-Shor period-finder
(not circular — the secret is extracted from the QFT distribution, not handed
back), but its oracle is a Python re-implementation: chord-only with `inv(0):=0`
on a toy field `p ≤ 41`. It is *deterministic and phase-clean*. The repo's own
`src/point_add/completeness_probe.rs` — the only code that runs the actual
`build()` on exceptional inputs — measures **probabilistic phase garbage** on
`dx=0` (corrupted on ~9/16 seeds, per `completeness_argument.md §2`). So the
recovery demo runs a strictly *gentler* adder than the one scored, and the
asserted equivalence (`inv(0):=0` "makes the adder exactly the circuit's
exceptional behaviour", ADR 0019) is unverified and contradicted by the repo's
own measurement. The scored 256-bit circuit is never run through a ladder or a
recovery (`ladder_full.rs` calls `build()` only to *count* gates).
**Severity: high** (claim framing).

### F5 — The "exact end-to-end mid-ladder bound" is exact only at toy scale
`analysis/verify/mid_ladder_bound.py` computes the exact `P[⋃ₖ Aₖ]` only for
`n = 1009/2003`; at `n ≈ 2²⁵⁶` it falls back to `28·2/n`, which *is* the
equidistribution value. `completeness_argument.md` ("equidistribution is no
longer load-bearing") is therefore not accurate for the number that actually
appears in the headline. The sharp `≈2⁻²⁵⁰` figure is additionally contingent on
the offset window encoding, which the *scored* circuit does not implement
(`build()` is a single point addition; the offset/ladder/∞-removal machinery is
analysis-layer only). Without it the bound is `≈2⁻¹¹`. Both clear Shor's ~1%, so
the conclusion survives — but the "exact, no-assumption" framing does not.
**Severity: medium** (claim framing).

### F6 — Chevignard 1098 qubits is misattributed to 256-bit
The repo cites "Chevignard 1098 q — the current qubit frontier" for secp256k1
(novelty-assessment, scientific-value, technical-report). Per the primary work,
**1098 is for P-224**; the **P-256 figure is 1193**. This should be corrected;
the honest gap to the frontier is different (and, on the paper's A2 bound of
1168, actually *below* the 1193 P-256 frontier — though the repo's own faithful
quantum-addend port at 1424–1680 is above it). **Severity: medium; factual.**

### F7 — "Published" vs "private" point-addition bounds
`README.md` labels the 2.7M/1175 and 2.1M/1425 figures as Google's *private*
Pareto points; `paper/technical-report.md` and `analysis/ecdlp_estimate.py` call
them the paper's *published* point-addition bounds. Babbush's public abstract
reports only full-ECDLP totals (`<1200 q/<90M`, `<1450 q/<70M`). Since the entire
"beats both published PA bounds" headline rests on these numbers, the wording
must be reconciled and the exact source (published table vs organizer-supplied)
stated. **Severity: medium; factual.**

### F8 — Internal metric-wording inconsistency ("peak qubits")
`README.md` calls the score's qubit count "peak qubits"; `scientific-value.md`
calls that same label inaccurate (`qubits = max_id+1`, `circuit.rs`). Register
reuse makes `max_id+1 = 1152 =` true peak (the `ladder_composition` Δ=0 result),
so the README is defensible and `scientific-value.md` is over-cautious — but the
two documents contradict each other and should be reconciled. **Severity: low.**

## Recommendation

Framed as the docs themselves recommend — a verified-artifact / reproducibility
methods contribution — this is publishable and valuable. But before submission
the Abstract/Contributions must be pulled back to match the buried caveats:

1. State that the machine-checked layer covers the **integer identity of one
   primitive's non-emitted variant** plus **≤64-bit** boolean lemmas (now
   extendable to 256/257-bit, F3) — not the measurement-based adders the scored
   circuit runs, and not by a proof bound to the emitter (Kani proves a copy).
   Well over 99% of the scored circuit's correctness still rests on the
   9024-shot sample.
2. Downgrade "computed + circuit-verified + demonstrated end-to-end attack with
   the incomplete adder this circuit implements" to what is true: a toy-scale
   ideal-Shor recovery using a re-implemented model adder, an amplitude *upper*
   bound exact only on toys, and a toy *predicate* detector.
3. Fix the Chevignard P-224/P-256 attribution (F6) and reconcile "published vs
   private" for the Babbush PA bounds (F7).

None of this touches the ECDLP's hardness. The engineering result is genuine; the
required change is to the rhetoric, not the artifact.
