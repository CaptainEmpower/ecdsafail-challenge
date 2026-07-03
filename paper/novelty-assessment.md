# Novelty assessment — is there a publishable delta?

*Working document. Assesses what this repo does that the current ECDLP
resource-estimation literature does not, to decide whether a preprint is
warranted and how to frame it. The comparisons below are now grounded in a
**full-text** pass (not abstracts): Babbush et al. arXiv:2603.28846 (57 pp.,
including the fuzz/ZK appendix), Han Luo et al. arXiv:2604.02311 (35 pp.), and
Chevignard–Fouque–Schrottenloher ePrint 2026/280 (EUROCRYPT 2026). Term counts
and quotes cited inline are from those extractions.*

## The landscape (2026)

ECDLP-via-Shor resource estimation for 256-bit curves is an active, expert-heavy
area with multiple 2026 results:

| Work | Headline (256-bit) | Correctness / completeness | Formal verification |
|---|---|---|---|
| Roetteler et al. 2017 (ePrint 2017/598) | ~2330 qubits, ~1.3·10¹¹ Toffoli | negligibility argument for exceptions (the framework this repo uses) | no (analysis) |
| Babbush et al. 2026 (arXiv:2603.28846, Google QAI) | `<1200 q / <90M Toffoli` or `<1450 q / <70M` | ~99% correctness via Fiat–Shamir **fuzz** (sampling, §5); the ZK proof is **"of Resource Costs" (App. A) — it validates the *cost bounds + honest fuzz execution*, not all-inputs circuit correctness**; full text has **no** "point at infinity" / "complete formula" (completeness absorbed by Shor's ~1%) | none (full text: `SMT/z3/model-check = 0`) |
| Han Luo et al. 2026 (arXiv:2604.02311, Tsinghua/Peking) | **1333 q** (prime 256-bit); `5n+4⌊log₂n⌋+O(1)` q, `O(n³)` Toffoli via refined Proos–Zalka EEA register-sharing — a genuine *space* advance | analytic; no explicit exceptional-case treatment (full text: `fuzz = 0`) | none (theoretical) |
| Chevignard–Fouque–Schrottenloher, EUROCRYPT 2026 (ePrint 2026/280) | **1098 q** (3.12n space) — the current qubit frontier | analytic | none (theoretical) |
| **This repo** | PA `1.36M Toffoli × 1152 q`; derived+measured full-ECDLP `~46M Toffoli / 1168 q` | **exact** end-to-end mid-ladder bound + **circuit-level** real-coordinate exceptional detector + offset ∞-removal | **z3 + Kani**, bound to the real `alloy` U256 type |

## What is genuinely differentiating

1. **Machine-checked arithmetic, bound to the implementation types.** The flagship
   papers establish correctness by sampling/fuzz (Babbush, §5 — 9024 Fiat–Shamir
   inputs, ≥99%) or leave it to analysis (Han Luo, Chevignard). None machine-checks:
   the full text of all three has zero occurrences of SMT / z3 / model-checking.
   This repo proves the load-bearing Solinas modular reduction and
   the peephole/adder/comparator identities over **all** inputs with z3 (22+
   lemmas, `unsat` on every negation), and re-proves the Solinas control flow with
   **Kani bit-precise BMC on the real `alloy_primitives::U256` type** against the
   actual secp256k1 prime — not an abstract model. *This is the clearest gap in the
   literature.*

2. **Completeness turned from argument into a computed+verified result.** Everyone
   in this lineage cites Roetteler-style negligibility for the incomplete affine
   adder. This repo makes it concrete: (a) a gating experiment measuring the
   circuit's actual behaviour on exceptional inputs; (b) an **exactly computed**
   end-to-end mid-ladder exceptional amplitude `P[⋃ₖ Aₖ]` over the real 28-window
   two-scalar ladder (`≈2⁻²⁵⁰` offset / `≈2⁻¹¹` standard, both `≪` Shor's ~1%);
   (c) a **reversible circuit** that detects the exceptional set on real `(x,y)`
   coordinates and is shown to match the scalar/dlog predicate on the whole group
   of several toy curves; (d) an offset-window encoding that removes the dominant
   zero-window ∞ term structurally; and (e) a **demonstrated recovery** — the full
   two-register Shor-ECDLP, run by exact statevector simulation on toy prime-order
   curves *with the incomplete adder + this handling*, **recovers the secret discrete
   log**, and the offset encoding is shown load-bearing for the *recovery* (dropping
   it collapses it), not only for the amplitude figure (ADR 0019). Nearly all prior
   work stops at (a-as-argument): Babbush's full text never mentions the point at
   infinity or a complete formula (the ~1% of exceptional/incorrect runs are simply
   absorbed by Shor's tolerance), and neither space-optimized paper treats the
   exceptional cases explicitly — let alone demonstrates a completeness-aware run.

3. **Emitted-and-measured full-ladder cost, not only a closed form.** The full
   28-window ladder Toffoli/depth/peak are stream-emitted and counted (no
   materialization), and the measured totals are consumed by the estimate — so the
   headline is corroborated by a measurement, and the read→add serialization depth
   is measured rather than assumed.

4. **Fully reproducible, open pipeline.** Byte-identical build, `just` recipes,
   deterministic analysis suite, ADR trail. Reproducibility at this granularity is
   uncommon for resource-estimate papers.

## What is NOT a strong claim (be honest)

- **Not a new algorithm.** The point-add uses known techniques (Cuccaro, Solinas,
  Gidney unary-iteration QROM, kickmix measurement-based uncompute) tuned hard. The
  full-ECDLP figure is **derived/composed** (measured PA × the paper's ladder),
  not an independent end-to-end circuit. It does not compete with the *algorithmic*
  space results (Chevignard 1098 q; Han Luo 1333 q).
- **Not the qubit-count frontier.** 1168 (A2) is above Chevignard's 1098 and Han
  Luo's 1333, and above Babbush's `<1200` on the Low-Gate side; the
  resident-quantum-addend port is measured wider still (1424–1680). The honest
  headline is *Toffoli-competitive with a much stronger correctness/completeness
  guarantee*, not *smallest*.
- **Toffoli×qubits is a competition metric**, not a physical cost; the defensible
  framing is the spacetime/cost-model mapping.
- **No demonstrated end-to-end attack *at scale*.** The full Shor-ECDLP pipeline
  *is* now demonstrated to **recover the discrete log** — but at **toy scale** (exact
  statevector simulation on prime-order curves of order 19/29/41, using the incomplete
  affine adder + offset/direct-lookup handling; `analysis/verify/shor_ecdlp_recovery.py`,
  run via `just recover`; ADR 0019).
  The 256-bit attack remains a *derived/measured cost estimate*, not a run — the toy
  demonstration + closed-form scale-up of the exceptional rate is the honest bridge,
  not an executed 256-bit break.

## Does this add value to cryptanalysis?

Separate the two senses of the word — the honest answer differs:

- **As *breaking* — no.** No new attack, no reduced qubit/gate frontier, no
  structural weakness in secp256k1. The ECDLP's hardness is untouched.
- **As *resource estimation / threat assessment* — yes, narrowly, to its
  *epistemics*.** The value is not a smaller number but a **more trustworthy** one:
  machine-checked all-inputs arithmetic (z3+Kani) where the frontier estimates use
  fuzz/analysis, and a computed + circuit-verified completeness treatment where they
  cite negligibility. The message a resource-estimation cryptanalyst takes away is
  *"these estimates can and should be machine-checked and completeness-verified —
  here is a template, and proof it costs nothing in competitiveness to do so."*

That is a reproducibility / standard-of-evidence contribution to the sub-discipline,
which is exactly why the framing below leads with rigor rather than "beats the
frontier."

## Recommendation

Publish, if at all, as a **methods / verified-artifact** contribution:
*"A formally-verified, completeness-rigorous, reproducible resource estimate for
the secp256k1 ECDLP point-addition primitive"* — with the aggressive optimization
as a case study and the z3+Kani+completeness rigor as the headline. Do **not** lead
with "beats the frontier."

**Full-text diff outcome (done).** The full text of Babbush App. A confirms their
ZK/fuzz is *not* a completeness proof — it is a zero-knowledge proof *of resource
costs* over a ≥99% fuzz sample, and the paper never treats the point at infinity or
a complete addition formula. Neither space-optimized paper (Han Luo 2604.02311;
Chevignard 2026/280) handles the exceptional cases or machine-checks arithmetic.
Items 1–2 above hold — and are sharper — against the full papers. Remaining
pre-submission items: settle authorship/attribution + disclosure posture and get an
independent reproduction of the byte-identical build.
